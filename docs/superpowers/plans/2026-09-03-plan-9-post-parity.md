# Plan 9 — the post-parity fix program Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **This plan reverses the port's founding rule.** Plans 1–8 reproduced the Java program's defects on purpose, and byte parity with the jar was the acceptance test. Plan 9 **fixes them**. There is **no `Compat` switch** (user directive): every fix lands directly and unconditionally, byte parity with the jar may break, and the evidence moves from "the jar agrees" to "the port's own golden did not move unexplained, and the routing got better on 605 real boards".

**Goal:** Land the 121 behavioural fixes catalogued in `docs/plan-9-prep/survey.md`, in the order the survey's §10 dependency analysis fixes, so that the port stops being a faithful reproduction of freerouting HEAD and becomes **a better router than any freerouting release measured**: the two ablation-proven Java regressions repaired (`benchmark/reports/java-regressions-2026-09.md` — small tier clean pass 0.60 → ≥ 0.77, DRC-clean 0.73 → ≥ 0.95), the optimizer stage doing work for the first time (#227), the T1 data-loss and non-termination rows closed, the DRC document and the result manifest telling the truth, and the settings/CLI surface predictable. The harness transitions from "the jar is the oracle" to "the port is the golden" (survey §7), the differential drivers whose only purpose was pinning a Java defect retire in the task that deletes the defect, and the ~260 Java-emulation shim occurrences that ten of those fixes make dead are collected and deleted.

**Architecture:** No new crate, no new module tree. Plan 9 edits **inside** the eight crates Plans 1–8 built (`fr-geometry`, `fr-board`, `fr-dsn`, `fr-settings`, `fr-drc`, `fr-router`, `fr-core`, `freerouting`) plus `scripts/`, `tests/reference/` and `benchmark/`. The dependency direction `freerouting → fr-core → fr-router → fr-drc → fr-board → fr-geometry` (spec §4) is unchanged, and no fix may invert it. Three structural changes are authorised and are named at their tasks: `BoardReadResult` gains a third variant (#91, Task 4), `apply_rules`' layer sentinel becomes a two-variant enum (#112, Task 4), and `ScoringSettings`' weights become non-optional in the type (#258, Task 21). Everything else is a body change behind an unchanged signature.

**Tech Stack:** Rust 2024, unchanged. `clap`, `serde`, `serde_json`, `thiserror`, `tracing`, `tracing-subscriber`. **No new workspace dependency except under ruling BK** (a stable named seeded RNG, and only if a fix needs one — see BK below). No `rayon`, no threads below `crates/freerouting`, no `unsafe`. `benchmark/` keeps its own Python/`uv` toolchain and is not a cargo dependency of anything.

**Spec:** **`docs/plan-9-prep/survey.md`** (845 lines — the reconciled catalogue: 24 task groups, the harness transition in §7, the de-Java-ification inventory in §8, the dependency constraints in §10, the 13 recommendations in §11). Its row details **are** this plan's content: every FIX row of §3–§6 appears in exactly one task below with its register id, its mechanism, its fix sketch and its Δrefs. Also binding: `docs/plan-8-handoff.md` (§3's 22 deliberate divergences, §4's parity-check inventory, §5's closed obligation register, §7's known limitations), `benchmark/reports/java-regressions-2026-09.md` (the R1/R2 evidence and the four secondary findings), `docs/java-quirks.md` (292 rows — the per-fix mechanism, and the register this plan adds a **status column** to rather than deleting rows from), `docs/cli-legacy-flags.md`.

**Later plans:** none scheduled. Performance work (real parallel passes) is explicitly **not** in this plan — user priority: *better routing beats speed*. If it is ever done it is a separate program with its own baseline, taken after Task 25's tag.

---

## Global Constraints

Every Plan 1–8 constraint and ruling remains in force **except** the ones the user's no-switch directive and the survey's §9.2 supersede, which are listed in "What is no longer binding" below. Additionally:

- **No compat mode (user directive).** There is no `Compat::{Java, Fixed}`, no `--mode=fixed`, no dual goldens, no per-fix gate on a switch. A fix lands **directly and unconditionally**. Byte parity with the Java jar may break, and where it breaks the breakage is **named in the commit that causes it**. Any task that finds itself wanting a switch stops and reports; a switch is a plan amendment, never a local decision.
- **Java no longer wins.** Plans 1–8's founding constraint is retired here, and its replacement is narrower: **Java is still the authority on what the Java program does** (every mechanism claim in this plan was read out of the clone at `/Users/em/Development/freerouting/freerouting` and the implementer re-derives it), but it is **no longer the authority on what the port should do**. A disagreement between this plan and the clone's HEAD about *the defect* is resolved for HEAD and reported. A disagreement about *the fix* is resolved by the survey's fix sketch, and a departure from that sketch is a reported plan amendment with its reasoning.
- **The clone is READ-ONLY.** `/Users/em/Development/freerouting/freerouting` is never written to by this plan. Upstream contributions are a separate activity; Task 25 emits the list, not the PRs.
- **`#![forbid(unsafe_code)]` at the top of every crate root, unchanged.** No fix weakens it, and no fix adds a `catch_unwind` boundary. Plan 6's five documented boundaries and Plan 7's one and Plan 8's one are the complete set, and they stay at seven. **A guard is not recovery** (Plan 7 ruling 7 / scan ruling 9, still binding): the §3.4 cluster adds the null test the sibling method already has, at the defect, and never a wrapper.
- **No new workspace dependencies, except under ruling BK.** `rand`, `rayon`, `slotmap`, `schemars` stay refused. BK's admissible set is `rand_chacha` / `rand_pcg` **and only if a fix needs real RNG infrastructure**; the port's existing bit-specified LCG, renamed, is expected to serve. Any addition is a `Cargo.toml` edit **plus** a recorded amendment in this file, never a silent edit.
- **No static mutable state.** The one recorded exception, `fr_geometry::Line`'s identity counter (ruling AE), is **scheduled for deletion** by this plan: #218 (Task 14) and #74 (Task 18) are its only two readers. If both land, Task 24 deletes the counter and the port has zero exceptions. If #74 is abandoned at Task 18, the counter survives with #74 named as the reason, and that is recorded in Task 25's report.
- **No threads below `crates/freerouting`, and no background waiters anywhere.** The two MCP spawn sites stay two. No task starts a process it then polls; every long run (a corpus sweep, a stem A/B) is run to completion in the foreground with its **full output redirected to a file** under `scripts/differential/out/` or `benchmark/baselines/`, and the file is read from disk. **Never `| tail`, never `| head` on a gate's output** — a truncated gate is an unread gate.
  **Carve-out, ruling BP3 — the three milestone benches.** `benchmark/scripts/remote-run.sh` launches the remote `bench run` **detached** (`setsid nohup`) and then polls it in its **own foreground loop** until it finishes and rsyncs the results back. That poll loop **is the run**, not a waiter wrapped around one, and the script's exit code is the remote job's. M1/M2/M3 are therefore **controller-executed steps** — the controller runs `remote-run.sh` in the foreground, redirects its full output to `benchmark/baselines/plan9-m<n>.log`, and hands the run-id back to the task. No implementer starts a background process, and no task polls anything itself. This carve-out covers `remote-run.sh` and nothing else.
- **Explicit-path staging.** Every commit stages the files it changed **by name** (`git add <path> …`). No `git add -A`, no `git add .`, no `git commit -a`. Golden regenerations touch hundreds of files; the commit that regenerates them lists the directories explicitly and its message says which fix moved them.
- **Quirk-register status-column discipline (recommendation 13, adopted).** `docs/java-quirks.md` **keeps every row** — it is a description of the *Java* program and that is what makes it useful upstream. Task 0 adds a **`status` column** with the values `pinned` (unchanged, the port still reproduces it), `fixed: T<n>` (the port no longer reproduces it; Plan 9 task `<n>` fixed it), `totalized`, `candidate`, `keep` (survey §9.1). A task that lands a fix **edits the status cell of every register row it closes, in the same commit**, and adds a `// fixed: T<n> (#id) — <one line>` comment **beside** the existing `// Java bug:` marker at each site — **with the row's own id in the parentheses**, which is what lets `crates/fr-core/tests/register.rs` check a row against *its own* markers and count one per site (amendment A17). **The `// Java bug:` marker is never deleted** (survey §9.1: it is the provenance that makes the register navigable, and it is what a future reader compares the jar against). Task 25's gate is that `grep -rn '// Java bug:' crates` is **≥ 165** (amendment A11 — `crates/*/src` answers 148; §9.1's figure is the `crates/` census) and every register row whose status is `fixed:` has a matching `// fixed: T<n> (#id)` marker.
- **Commit-message verification blocks.** Every commit message ends with a fenced block containing the **actual output lines** of the gates that ran, not a claim that they ran:
  ```
  gates:
    cargo nextest run --workspace      -> <N> passed, <M> skipped
    cargo clippy --workspace --all-targets -- -D warnings -> clean
    directed: <test name>              -> fail-before / pass-after
    port goldens (G1 stems)            -> <k> drivers, 0 diffs   [or: regenerated, see below]
    goldens moved: <family> <count files> because <fix id>: <direction>
  ```
  A commit that moved a golden and does not carry the `goldens moved:` line is rejected at review. "The numbers went up" is not a reason (survey §7.4 rule 4); the line names the fix and the direction.
- **The evidence bar, per fix** (survey §2.1, minus the mode axis): **(1)** the register id (or the new id this plan allocates), **(2)** the observable effect in a user's terms, **(3)** a **directed test that fails before the fix and passes after** — the fail-before is run and its output pasted, **(4)** a green full run, and **(5)** **where the fix moves a committed reference, an A/B number** — a **U-only** fix (one the corpus does not reach) instead carries **the directed test plus the stated latency argument in its register row**, which is the full bar for that row and is not a shortfall (ruling BP10). Items (1)–(4) are unconditional. The §3.4 ten-site guard cluster (Task 6), #211+#45 (Task 5), #213 (Task 9), #148 (Task 13), #194's argument half, #258 and most of Task 21 land on that second form, and each says so at its row.
- **Commit trailer**, on every commit:
  ```
  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_015f1ubhBBpiWHAooDsso97Z
  ```
- **Branch and merge.** All work lands on `plan-9-post-parity` off `main` at `c467758`. Nothing is committed to `main` until Task 25. The plan document itself is **not** committed by the drafting session.

### What is no longer binding (survey §9.2, adopted)

The roadmap's `Compat::{Java, Fixed}` design and its whole mode axis; §7.1/§7.2's release split; "byte parity is the acceptance test"; "do not fix these in the port until parity is proven" (`docs/java-quirks.md`'s first rule — Task 0 rewrites it); the Plan 8 scan's R3/R15 **additive-only** constraint on `fr-router` (a Plan 8 constraint that does not survive into Plan 9 — see #251, Task 20); the mandatory `-XX:hashCode=2` and `-Duser.language=en -Duser.country=US` JVM flags (survey §7.3 — both are jar-side workarounds for #144/#145, which the port does not have, and they disappear with the drivers that needed them).

---

## Controller rulings binding this plan — the BL series

The survey's **13 recommendations (§11) are ADOPTED AS WRITTEN** and are not restated as separate rulings; each is cited at the task that executes it. On top of them:

- **BL1 — No compat mode.** Stated as a Global Constraint above. Supersedes roadmap §1.1 and every "inverts in `Fixed` mode" note in the catalogue.
- **BL2 — Three gating tiers, and only three.** Per fix, per task group, and three whole-corpus milestones. Spelled out verbatim in "The three gating tiers" below. **No task invents a fourth gate**, and no task runs the full bench outside a milestone: a 605-board run costs hours and a plan that runs one per fix would be a plan nobody finishes.
- **BL3 (ruling BK, as amended) — the RNG.** Any generator the port carries must be a **stable, named, seeded algorithm** with cross-version **and** cross-platform value stability. `StdRng` and `SmallRng` are **excluded outright** (neither guarantees value stability across `rand` versions). The port's existing bit-specified LCG satisfies the rule already: **keep the implementation and rename it `SeededLcg`** (`crates/fr-geometry`), dropping only the claim that it reproduces the JVM. A hand-rolled **splitmix64** is an acceptable substitute for the single shuffle that actually needs one. `rand_chacha` / `rand_pcg` are admissible **only** if a later fix needs real RNG infrastructure, and adding either is a recorded amendment. Quirk #30's per-call construction stays. Task 24 owns the rename; Task 13 owns the `delaunay.rs` private copy.
- **BL4 — Sequence.** The plan is a **sequential writer**: one task at a time, one fix per commit inside a task, one regeneration at the end of each task. Tasks 3–7 touch different crates and their internal fix commits may be reordered by the implementer, but the *tasks* do not overlap and no two tasks hold the tree at once. Task 1 lands before any behavioural fix. Task 2 lands immediately after.
- **BL5 — The de-Java-ification tasks land after the routing-quality tiers.** Tasks 23 (DJ1) and 24 (DJ2) sit with the T4 tail, after Task 22 and before Task 18. Shim hygiene changes no routed board and is therefore last; it does not compete with a routing fix for a slot.
- **BL6 — New register ids.** The register's next free id is **#293** (`docs/java-quirks.md:610`, `:889`, re-read at Task 0 before allocating). Recommendation 7 is adopted: **R1 = #293, R2 = #294, I1 = #295, I2 = #296**, each with `benchmark/reports/java-regressions-2026-09.md` as its evidence column. Any further new row claims the next free id in write order (ruling Z, carried forward).
- **BL7 — Retire a driver in the task that deletes the behaviour it pins** (recommendation 2). Never a sweep at the end: a driver retired early loses the evidence that its task is complete. "Retire" means **run it once more, record the final MATCH count in this plan's amendment log and in Task 25's report, delete the pair, and keep the *Rust* half's assertions as unit tests wherever they carry a literal worth keeping.**
- **BL8 (as amended by ruling BP1) — The frozen baseline is written once and never read again by anything.** The freeze lives at **`tests/reference-frozen/java-head-2026-09/`** — a **sibling** of `tests/reference/`, never a child of it — and it is a historical artefact, not a test input. **No test and no script reads it**, `scripts/quality-ab.sh` included: the G2 jar reference numbers are derived from it **once**, at Task 1, and written **outside** it to `benchmark/baselines/quality-baseline-java-head.tsv`. `grep -rn "reference-frozen" crates/ scripts/` must return **nothing but prose** for the whole plan. `tests/reference-frozen/README.md` says so.
- **BP11 — the global Files note.** Every task's `**Files:**` line is read as **implicitly** carrying two more entries, so they are not repeated 26 times: **(a)** `docs/java-quirks.md` — 23 of 26 tasks edit a status cell, and the edit is staged by name in the same commit as the fix; **(b)** the golden families the task's own "Evidence / acceptance" paragraph says it regenerates (`tests/reference/<family>/**`), staged by directory, by name, in the regeneration commit. Explicit-path staging (the Global Constraint) applies to both: implicit in the Files line never means `git add -A`. Everything else a task touches is listed explicitly, and a task that finds it must edit a file neither listed nor covered by this note **stops and reports** — it is an amendment.

---

## The three gating tiers (verbatim — BL2)

**Tier G1 — per fix, every commit. Fast.**

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo test -p fr-router --test batch_parity
scripts/differential/run.sh p6t1                 # 5 boards / 6 rows, against the port golden
scripts/differential/run.sh p8t1                 # 10 rows, against the port golden
                                                 # (no lane argument — the CI lane is the
                                                 #  default; `p8t1 ci` is a stem-name error,
                                                 #  amendment A16)
scripts/differential/run.sh p8t2 e2e             # the manifest, field for field
scripts/differential/run.sh p8t3 e2e             # the DRC document, score, exit code
scripts/differential/run.sh p7t9                 # the pipeline's per-pass tuple
```
plus **the fix's own directed test, named in the task, run once before the fix to record its failure and once after**. Full output of every driver goes to `scripts/differential/out/<driver>.txt` and is read from the file.

**The G1 rule, amended to BL4's cadence (ruling BP9).** BL4 makes a task *one fix per commit, one regeneration at the end*, so a mid-task commit that changes behaviour **necessarily** shows golden diffs, and the old "zero diffs on every commit" rule would paint every one of them red. The rule is therefore **two rules**:

* **Per commit (inside a task):** `fmt`, `clippy`, `cargo nextest run --workspace` and the drivers **that the commit does not move** are green; the commit's own directed test passes; and the commit message states the golden delta it knowingly leaves outstanding — `pending goldens: <family> <count files> because <fix id>: <direction>` — for the task's regeneration commit to discharge. A commit that leaves an *unstated* delta is rejected at review.
* **At task close:** the regeneration commit re-cuts the named families, carries the aggregate `goldens moved:` block (one line per fix, with its direction), and the **whole** G1 list above is green with **zero diffs against the committed goldens**. A task does not close otherwise.

The reviewer's check is unchanged in substance and gains one clause: every `pending goldens:` line inside the task is matched by a `goldens moved:` line in its final commit, and the number of commits touching `tests/reference/` is still the number of `goldens moved:` blocks.

**Amended again by ruling BT (Task 1): a family regenerates only when a fix MOVES it.** The plan originally had Task 1 re-cut all five families from the port as its closing act, on the reasoning that #234 would tighten every multi-pass stem further. **Task 1 measured that reasoning to be false** — zero golden movement from all three of its fixes — which leaves the regeneration as a pure *lane switch* (jar-cut references become port-cut, and quirk #92's `hostCad`→`host_cad` head tokens move with them) bought at the price of retiring live parity coverage. So:

* **A task regenerates a family only when one of its own fixes actually moves that family.** "The plan said this task regenerates X" is not a reason; a measured diff is. A task whose fixes move nothing writes `goldens moved: NONE` and says why, and that closes its regeneration obligation.
* **The #92 lane switch lands per family, the first time that family regenerates for cause.** It is not a separate event and never a sweep. The task that first moves a family carries, in the same commit, the mechanical consequences: the `hostCad`→`host_cad` head-token churn, the three `references_are_from_the_head_jar` provenance tests made lane-aware off `meta.txt`'s own `lane` line, and — for the D family — quirk #146's counts and uuids transcribed into `natural_tone_preamp_is_the_reference_minus_three_dangling_tracks` as literals rather than read out of a file (this tree's standing pattern: a test that reads its expectations out of a file can agree with itself while disagreeing with the jar).
* **The one unrepairable case is pre-resolved** (ruling BT, and see amendment A21): `crates/fr-core/tests/stats.rs`'s host-scrape row needs a **jar-written, camelCase** `.ses` to exist somewhere the test may read, and after the B/C lane switch no such file remains under `tests/reference/` (measured: 20 files carry `hostCad` before, **0** after). At that future regeneration the needed `hostCad`-bearing reference files **migrate to committed directed fixtures under `crates/fr-core/tests/data/`**, carrying a provenance header saying they are the pre-lane-switch real-corpus files and naming the commit they were cut at. BL8 stays intact — a test data file is not the frozen tree, and nothing reads `tests/reference-frozen/` — and the quirk exercise survives. The `p8t2` transcript is re-cut against the migrated fixtures in the same commit.

**Tier G2 — per task group. The stem quality A/B.**

`scripts/quality-ab.sh <task-id>` (Task 0 writes it) runs the port over **the 8 batch stems + the 13 CLI stems + the 8 DRC stems**, at each stem's committed `-mp` cap, with **`RouterBudget::disabled()` on both sides** (ruling AI survives the switch: time is out of every *quality* measurement), and prints one row per stem for two references at once:

* **the jar column** — the frozen HEAD numbers in **`benchmark/baselines/quality-baseline-java-head.tsv`** (Task 1 derives it once from the frozen references and writes it **outside** the frozen tree — BL8 as amended by BP1). **Quality only**; it carries no timing, and it is **context, never a gate**.
* **the port column** — the **previous task's** own tsv, `benchmark/baselines/ab/quality-ab-T<n-1>.tsv`, which is what every rule below is actually scored against. Each task commits its own `benchmark/baselines/ab/quality-ab-T<n>.tsv` (and its per-fix rows) at task close (ruling BP15), so the rolling port baseline is a committed artefact a later task reads rather than re-derives, and Task 14's joint neckdown table **reads Tasks 2/10/14's committed rows** instead of reconstructing them from stale goldens.

**Timing joins acceptance (ruling BO, encoded by ruling BP4).** `quality-ab.sh` emits a **`cpu_s` column per stem** beside the quality columns, taken as the **median of 3 repeats on an otherwise-quiet machine** (the spread is printed with it; it is the only noise figure this tier has). The rolling port timing baseline is **`benchmark/baselines/stem-times.tsv`**, seeded at Task 1 from the **`v1.0.0-rs` per-board `cpu_s`** already in `benchmark/results/v1.0.0-rs/`, and **updated at each task's close** from that task's own tsv. The rules:

* an **equal-quality** change **must not be slower** — a slowdown with no quality gain is rejected or reworked, not accepted;
* a **quality-winning** change may cost time, but **> 2× on any single stem** or **> 20 % on the corpus median** escalates to the controller for an explicit keep/rework ruling before the task closes;
* every milestone additionally reports the **corpus-median `cpu_s` ratio against the prior milestone** (M1 against `v1.0.0-rs`), so no drift is invisible.

**Four tasks are named in advance as the ones expected to trip the escalation**, and each carries a pre-authorised escalation line at its acceptance: **Task 9** (#227 — a whole optimizer stage begins working, one board deep copy per item), **Task 1** (#234 — the pull-tight is no longer abandoned on wall-clock), **Task 12** (seven constant-time stubs become real geometry) and **Task 13** (an exact `BigInt` in-circle predicate in the ratsnest path). Priority order is unchanged: **hard routing metrics first, speed second.**

**The gate version (ruling BP8).** Every `quality-ab.sh` run stamps a **`gate-version`** on its tsv header. It starts at **`g1`**; **Task 13 bumps it to `g2`** (#82+#147 redefine `incomplete_count`) and **Task 19 bumps it to `g3`** (#195+#196 make the length breakdown and the bounding box legal inputs). **An A/B is only ever compared within one gate-version.** The task that bumps it **re-cuts the affected baseline columns in the same commit** — it re-runs the previous task's stems under the new gate and commits both the old and the new `benchmark/baselines/ab/quality-ab-T<n-1>.tsv` rows — so the next task's comparison is like-for-like. Task 13's "report `incomplete_count` both ways" **is** that re-cut, written down.

| metric | source | rule |
|---|---|---|
| incomplete connections | the **referee's** DRC, never the manifest (survey §4.1's 45–63 % disagreement) | **must not rise on any stem; must fall on at least one** — *except on an escalated task; see the BP12 arm below* |
| clearance violations | referee DRC | **0 on every stem, always** |
| normalized score | `fr_router::score::normalized_score(&ScoringSettings)` — an `f32` throughout, deliberately (survey §9.1) | must not fall by more than the `f32` noise floor |
| total trace length | `traces.total_length_mm` | reported; ↓ preferred |
| via count | `vias.total_count`, split through-hole/blind/buried | reported; ↓ preferred — **I2 is watched here** |
| bend count | `bends.total_count` | reported |
| **`cpu_s` per stem** | the driver's own CPU time, **median of 3 repeats**, against `benchmark/baselines/stem-times.tsv` (the **previous task's** number — never the jar's) | **equal-quality: must not be slower.** Quality-winning: tolerated, but **> 2× on a stem** or **> 20 % on the corpus median** escalates to the controller before the task closes. |

**The BP12 arm on "incompletes must not rise" (ruling BU(b)).** **On an escalated task the M-bench
is the gate, G2 records.** The rule is a parity-era rule: a rise in incompletes used to mean the
port had diverged from the jar. Once the port's answer is allowed to be *better* than
freerouting's, the rule cannot express a **trade of connectivity for legality** — which is what R2
(#294) is, and what several later rows will be: a connection that can only be closed with an
illegal trace should fail instead of being closed illegally, so the honest incomplete count rises
and the DRC-clean rate rises with it. So when a task has raised a **BP12 escalation** and the
controller has adjudicated it against a **milestone bench** (M1/M2/M3, 605 boards, against the
frozen `java-278fe14` view), the milestone's verdict is the acceptance decision and G2's flags are
**evidence inside it**, never a veto over it. Nothing is softened: `quality-ab.sh` still prints the
flag, still names the stem and still exits non-zero, and a task still never decides for itself that
a flag is acceptable. What the arm settles is only *who decides* — the controller, on the bench.
**An unescalated task with the same flags is still a stop-and-report**; this is not a general
licence for incompletes to rise. The precedent is Task 2: 10 rows over 6 stems flagged, 372 → 2
sub-minimum traces and 0 clearance violations against them, escalated with the numbers, adjudicated
at M1 (corpus clean-pass 0.375 → 0.550, small-tier DRC-clean 0.727 → 0.958), **accepted with the
residual gap recorded by ruling BV**.

**Forbidden inputs to G2 until their fix lands:** `traces.total_{vertical,horizontal,angled}_length` (they do not sum to the total — #195, Task 19) and `board.bounding_box.width`/`height` (they hold the lower-left corner — #196, Task 19). `incomplete_count` is itself a fix target (#82, #147 — Task 13): **while those are in flight, report it both ways on the same board**, or the improvement is indistinguishable from the metric moving underneath.

**Tier G3 — the full 605-board `pcbench` bench. Exactly three runs in the whole plan (rulings BN and BP3).**

**This block is the measurement spine, and it is written from the harness as it actually is** — `benchmark/scripts/remote-run.sh`'s header, `benchmark/bench/cli.py`'s `run` and `compare` signatures, and the two runs of record in `benchmark/results/`. Every milestone below invokes exactly this, changing only `<n>`:

```
# 1. On the workbench, rs-only, one unique run-id per milestone.
#    remote-run.sh rsyncs benchmark/ up, launches `bench run` detached on the host, polls it
#    in its own foreground loop, and rsyncs results/<run-id>/ back (Global Constraint carve-out).
#    Everything after `--` is passed straight to `bench run`; a --candidates-file given there
#    wins over the $BENCH_CANDIDATES the script exports.
cd benchmark
scripts/remote-run.sh --remote-dir freerouting-rs/benchmark --jobs 12 --poll-interval 300 -- \
  --candidates rs-main --candidates-file candidates.rs.toml \
  --tier pcbench --max-passes 10 --timeout 300 --threads 1 --run-id plan9-m<n>

# 2. Locally, against the FROZEN java run of record. --runs is what makes the comparison use
#    the frozen view instead of "the latest run per candidate".
uv run bench compare --baseline java-current --against rs-main \
  --runs java-278fe14,plan9-m<n> --tier pcbench --out plan9-m<n>-vs-java-278fe14
```

**Why every token of that is what it is:**

* **`--candidates rs-main` only, and `java-current` is NEVER re-run** (ruling BN). The Java side is the frozen `java-278fe14` run — 605 boards, `seeds 1`, `-mp 10`, `timeout 300`, `threads 1`, `jobs 12`, taken on `workbench` on 2026-09-02 against jar sha `278fe14123c4`. Re-running it costs ~2h10 and would compare the port against a *different* Java measurement than the baseline of record does.
* **`--tier pcbench`.** `bench/corpus.py::select` tests **one** tier string for membership in a board's tier list. The manifest's tiers are `pcbench`, `d3-a/b/c`, `regression`, `kicad-fixtures`, `dac2020`, `hard`, `canary`. **There is no `small`, `medium` or `large` tier** — those are prose labels for the D3 buckets in `reports/java-regressions-2026-09.md`, and passing them selects **zero boards**. Where this plan's acceptance thresholds say "small tier", they mean **the D3-small subset of the report**, read out of the compare's per-board rows, not a `--tier` argument.
* **`seeds 1`** (the default, and what both runs of record used), **`--max-passes 10`, `--timeout 300`, `--threads 1`, `--jobs 12`** — the `v1.0.0-rs` / `java-278fe14` argument set, so `compare` accepts the two runs as compatible.
* **`--candidates-file candidates.rs.toml`.** `remote-run.sh` exports `BENCH_CANDIDATES=$PWD/candidates.remote.toml` on the host; `candidates.remote.toml` names nine historical jars that live only in the old `~/freerouting-bench/binaries`, and `load_candidates` validates **every** exec in a file before returning, so it fails before routing a board. `candidates.rs.toml` is the **two-candidate, Java-clone-independent** file (`java-current` → in-repo `binaries/freerouting-current-executable.jar` + `sha_file`, `rs-main` → `../target/release/freerouting`), it is what BN's shape needs, and **Task 0 commits it**. Ruling: **KEEP it; do not delete it and do not re-couple the milestones to `candidates.toml`**, whose `java-current` resolves into the read-only Java clone's build output.
* **The Rust binary is outside the rsync scope.** `remote-run.sh` syncs `benchmark/` only; `rs-main`'s `../target/release/freerouting` resolves on the host to `~/freerouting-rs/target/release/freerouting`, which nothing in the script puts there. **Each milestone therefore carries an explicit out-of-band step**: build the branch at the milestone commit on the workbench (or scp the binary), and **paste the sha `bench` records in `results/plan9-m<n>/meta.json` next to the branch sha it must equal**. A mismatch invalidates the run.
* **`--runs java-278fe14,plan9-m<n>`.** Without `--runs`, `compare` silently takes "the latest run per candidate". With it, it reads exactly the frozen java view and the new rs run — which is precisely how `reports/v1.0.0-vs-java-clean.json` (`runs: ["java-278fe14","v1.0.0-rs"]`, `baseline: java-current`, `against: ["rs-main"]`) was produced.
* **Time metric.** `--time-metric` defaults to `auto`, which falls back to **CPU time** whenever a compared run used `--jobs > 1`. At `--jobs 12` that is automatic, and `cpu_s` is what the baseline of record reports; pass `--time-metric cpu` explicitly if a run is ever taken at `--jobs 1`.
* **`java-278fe14` is a synthetic view and `benchmark/results/` is gitignored**, so it must be **reconstructible**. Task 0 writes **`benchmark/scripts/make-java-view.sh`**, which rebuilds it in two steps: (1) create `results/java-278fe14/` holding a **`meta.json` copied from the source `full-rs-vs-java` run with `run_id` rewritten to `java-278fe14` and the `candidates` array filtered down to the single `java-current` entry** (`compare.collect` iterates `meta["candidates"]` and takes only the names it was asked for — filtering that array *is* the whole trick; the `cells` list may stay as it is); and (2) **symlink `results/java-278fe14/java-current` at the frozen run's `java-current` cell directory**. The script takes the source run directory as its argument, is idempotent, and **verifies** afterwards that the view's `candidates[0].sha` is `278fe14123c4` and that its board count is 605.
* **Outputs.** `bench run` writes **`benchmark/results/plan9-m<n>/`** (gitignored — the run is identified by its run-id and its `meta.json`, and the *compare* is the artefact). `bench compare` writes **`benchmark/reports/plan9-m<n>-vs-java-278fe14.{json,md,html}`**, and the milestone commit adds the **`.json` and `.md` with `git add -f`** (`benchmark/.gitignore` excludes `reports/*.json`, `*.md` and `*.html`, and its own header says a report worth sharing is added deliberately with `git add -f reports/<name>.*`). `remote-run.sh`'s full console output goes to `benchmark/baselines/plan9-m<n>.log`, read from the file.

**Rule:** **`overall.verdict` is `better` or `same`, with zero `hard_losses`.** The referee's hard metrics are `clean_pass_rate`, `unrouted`, `violations`, in that order, and a `better` verdict is impossible with any hard loss. What makes this the right instrument: the referee scores the `.ses` **independently** with KiCad 10 DRC and each board's own rules, so it cannot be fooled by the manifest rows Task 19/20 are fixing. **On noise, precisely** (correcting an earlier draft): at `seeds 1` `compare` computes a **zero** noise band — `reports/v1.0.0-vs-java-clean.json` records `noise: 0.0` for every metric on every board — so **there is no seed-stdev protection and this plan does not claim one**. The protection is different and stronger for the hard metrics: **the router is deterministic**, so a hard-metric delta at `seeds 1` is exact, not sampled. Timing is the metric that *is* noisy at one seed; it is read as the **corpus-median `cpu_s` ratio** (BO/BP4), never per board, and never as a verdict input at this tier.

**Every milestone additionally records the corpus-median `cpu_s` ratio against the prior milestone** — M1 against the `v1.0.0-rs` run of record, M2 against M1, M3 against M2 — with the BO thresholds (> 20 % corpus-median regression escalates) applied to the ratio.

**The three milestones, named:**

| milestone | runs at the end of | what it proves |
|---|---|---|
| **M1** | **Task 2** | the two ablation-proven regressions are repaired in the port: on the report's **D3-small subset** clean pass ≥ 0.77, connected ≥ 0.79, DRC-clean ≥ 0.95; on the D3-large subset clean pass ≥ 0.30. This is also the run that **re-baselines every later A/B**. Its **pre-fix position needs no new run**: the baseline of record is the existing **`v1.0.0-rs`** run (`benchmark/reports/v1.0.0-vs-java-clean.json` — 573W/32L/0T, 15 hard losses), and `c467758`/`eff706a` are **documentation-only commits on top of v1.0.0**, so the binary that produced it is the branch's own base. |
| **M2** | **Task 16** | the T2 heart — rooms/doors (8), the optimizer stage (9), shove and clearance (10), geometry (11), airlines (13), caches (14), fanout (15), the via optimizer (16) — is `better` or `same` with zero hard losses, and I2's via-inflation column has moved or is explained. |
| **M3** | **Task 18** (the last behavioural task) | the whole branch. Read into Task 25's completion report. Any group-18 row that shows a hard loss here is **reverted at M3** — each of those six rows is individually abandonable by construction. |

**The acceptance rule, restated for a world without modes** (survey §7.4): a fix lands if **G1 is green**, **G2 shows no hard loss**, and **a named, reviewed explanation exists for every golden that moved**. A fix that improves the score but cannot explain its geometry churn **does not land**. A fix that makes the score *worse* and is still correct — #82 raising the honest incomplete count is the standing example — **lands anyway**, with the reason recorded: the score is a proxy, and the direction is better routing.

---

## The harness transition — what moves, what dies (survey §7)

| code | family | files | after Plan 9 |
|---|---|---|---|
| **G** | geometry + DSN round trip | `tests/reference/<board>/*` for the 6 rows of `fixtures.txt`, plus `crates/fr-dsn`'s round-trip goldens | **regenerated from the port**; the 2.3.0 Specctra spelling is kept as a *decision* (#92 is a real HEAD bug), not as a jar comparison |
| **R** | per-connection router transcripts | `tests/reference/router-*/router.jsonl`, `router-steps18.jsonl`, 6 rows | **regenerated from the port**, by running the port's own `p6t1` twin, so "the reference and the differential describe the same run" survives |
| **B** | whole-board batch SES | `tests/reference/<stem>/batch.ses` + pass transcripts, 8 stems | **regenerated from the port**; unchanged in form and still the strongest determinism gate in the tree |
| **C** | CLI end to end | `tests/reference/cli-*/{argv.txt,route.ses,route.exit,route.log,manifest.json,drc.json,meta.txt}`, 13 stems | **regenerated from the port**, keeping `--verify-hash-modes`' *shape* as a port-side two-run identity check |
| **D** | DRC documents | `tests/reference/drc-*/*`, 8 stems | **regenerated from the port**; a pure function of the board, the cheapest family to re-cut |
| **X** | differential drivers | `scripts/differential/`, 34 `run.sh` cases + 5 sweeps + 26 probes | see below |
| **U** | unit and directed tests | ~2 389 tests | **inverted in place, per fix** — this is where a fix's own evidence lives |

**Drivers that retire, each in the task named** (BL7): `p8t5` + `sweep-p8t5.sh` → **Task 22** (#259/#262/#131-#135 delete the bug-for-bug legacy parsing the 87 rows pin); `p8t1probe` → **Task 22** (#246+#242) with its `java_shift_loop_hangs` predicate; `p8t2probe` → **Task 20** (#247/#248/#251); `p8t6` → **Task 25** (the eleven-row MCP delta table is asserted to be exactly itself and only changes if a Plan 9 fix reaches the MCP surface — recommendation 12: CLI first, MCP only where the tool already exposes the neighbouring option); `p2t13`'s Delaunay-order modes → **Task 13** (#82); `p6t2` → **Task 8** (#159); `p4t1`'s 64-case settings matrix → **Task 21**.

**Drivers that convert to port-golden comparison** (they are the only per-pass/per-connection instruments in the tree and are valuable independently of what they are compared against): `p8t1`, `p8t2 e2e`, `p8t3 e2e`, `p6t1`/`gen-router-reference`, `p7t9`. Each keeps its Rust half, drops its Java half, and diffs against the committed golden. **`run.sh` keeps an `--against-jar` escape hatch** for as long as a jar is buildable — it costs nothing and it is how a surprising golden churn gets triaged.

**Kept as a live jar comparison: nothing.** Once the port's answer is allowed to be better, a live jar diff produces a red run for every fix, and a harness that is red by design is a harness nobody reads.

**The five fixture gaps, each budgeted as a task-sized item** (survey §7.5, recommendation 9 — ground truth now means *a hand-computed or KiCad-DRC-verified expectation*, reviewed in the task, not the jar's answer):

| fixture | gates | owned by |
|---|---|---|
| a **90-degree** board | #159 | Task 8 |
| a board with **per-layer trace widths** | #187, #128 | Task 11 (built), Task 21 (reused) |
| a **≥ 6.71 M-unit or circular** outline | #94+#89+#93 — **check `tests/reference/cli-large-outline` first, it may already serve** | Task 4 |
| a board with **multi-net SMD pins** | #194 | Task 13 |
| a board with **signal-layer pours** and a foreign-net trace | #50 | Task 10 |

Two further synthetic fixtures are owned by Task 6 (#185's resampled polyline whose two ends meet; #181's two parallel lines with a null intersection) and one by Task 18 (#182's acid trap: a trace approaching a same-net pin at an acute angle).

---

## Interfaces — the cross-task index

Signatures this plan **changes or adds**. Everything not listed keeps the shape Plans 1–8 gave it, and a task that finds otherwise stops and reports.

```rust
// crates/fr-router/src/pipeline/airline.rs — Task 2 (R1). The roster carries FIVE `// not ported:`
// lines at :21-25; exactly THREE of them become ported methods — `calculateItemDistance` (:23),
// `calculateMinDistance` (:24) and `getItemReferencePoint` (:25). The other two,
// `nearestPointOnTrace` (:21) and `findClosestPointsBetweenTraces` (:22), STAY unported and keep
// their roster lines. The three greps are re-stated in the same commit.
pub fn calculate_item_distance(board: &Board, item: ItemId) -> f64;      // AutorouteAirlineCalculator.java:162-177
fn calculate_min_distance(/* … */) -> f64;                               // :179-202, private
fn item_reference_point(/* … */) -> FloatPoint;                          // :204-213, private

// crates/fr-router/src/pipeline/batch_autorouter.rs — Task 2 (R1) then Task 9 (#213).
// THE TREE HAS TWO FUNCTIONS HERE AND THEY STAY TWO (ruling BP5). `autoroute_items_with_handled`
// is the p7t1 differential seam and must keep its shape; `autoroute_items` is the thin wrapper.
// Neither takes `&mut self`, and no task may conflate them into one `&mut` signature.
pub fn autoroute_items(&self, board: &Board) -> Vec<ItemId>;                          // :702, wrapper
pub fn autoroute_items_with_handled(&self, board: &Board) -> (Vec<ItemId>, BTreeSet<ItemId>); // :714, the seam
// Task 2 (R1): both return the work list SORTED ascending by `calculate_item_distance`, with the
// existing descending-id walk surviving as the stable tie order. No element type changes.
// Task 9 (#213): the element type widens to carry the qualifying net, in BOTH, and the seam's
// second component is untouched:
pub fn autoroute_items(&self, board: &Board) -> Vec<(ItemId, NetIndex)>;
pub fn autoroute_items_with_handled(&self, board: &Board) -> (Vec<(ItemId, NetIndex)>, BTreeSet<ItemId>);

// crates/fr-router/src/autoroute/path/inserter.rs — Task 2 (R2).
// The candidate list at :509 is filtered against the BOARD RULES' minimum, never the board's
// running minimum over already-inserted traces.
fn insert_fanout_micro_neckdown(&mut self, /* … */) -> InsertResult;     // uses fr_board::BoardRules::get_min_trace_half_width

// crates/fr-router/src/pipeline/stop.rs — Task 1 (#234). `RouterBudget` is DECLARED at
// stop.rs:493 and re-exported as `fr_router::pipeline::RouterBudget`; `crates/fr-core/src/ctx.rs:42`
// merely HOLDS one (`pub budget: fr_router::pipeline::RouterBudget`, built at :56). The edit is in
// stop.rs, which Task 1's Files line names (ruling BP5). It is the TRAIT impl, not an inherent fn:
impl Default for RouterBudget { fn default() -> RouterBudget; /* opt_changed_area_ms = 0 */ }  // :522
// `RouterBudget::disabled()` (:542) is unchanged. The value becomes a real setting:
// `router.opt_changed_area_ms`, default 0, read where ctx.rs builds the budget.

// crates/fr-dsn/src/lib.rs — Task 4 (#91). A third variant, not a hard failure.
pub enum BoardReadResult { Ok(Board), Partial { board: Board, diagnostic: ReadDiagnostic }, Err(DsnError) }

// crates/fr-dsn/src/rules_reader.rs — Task 4 (#112). The -1 sentinel becomes unrepresentable.
pub enum RuleLayerScope { AllLayers, One(usize) }
fn apply_rules(&mut self, /* … */, scope: RuleLayerScope) -> Result<(), RulesError>;

// crates/fr-router/src/pipeline/stop.rs + optimizer.rs — Task 9 (#214, #227, #202).
// The exit reason leaves the loop; the optimizer gets a STAGE-SCOPED stop. The three-state
// `RouterStop` is KEPT (survey §9.1) and must not collapse to a bool.
pub enum BatchLoopExit { Completed, MaxPasses, Stagnated, StopRequested, Deadline }
impl BatchLoopResult { pub fn exit(&self) -> BatchLoopExit; }
impl RoutingPipeline { fn run_optimization_stage(&mut self, /* … */); /* resets the stage flag on entry */ }

// crates/fr-geometry — Tasks 12, 13, 24.
impl PolygonShape { pub fn contains_on_border(&self, p: FloatPoint) -> bool;      // #28, was a `false` stub
                    pub fn area(&self) -> f64;                                     // #26, was always 0
                    pub fn cutout(&self, /* … */) -> ConvexShapeList;              // #29
                    pub fn intersects_polygon(&self, other: &PolygonShape) -> bool; } // #27
pub struct SeededLcg(/* the bit-specified LCG, renamed — BL3 */);                  // Task 24

// crates/fr-drc + crates/fr-router/src/score — Tasks 19, 20.
impl Item { /* `smallest_clearance` FIELD DELETED — #153 */ }
impl ClearanceViolation { pub fn smallest_clearance(list: &[ClearanceViolation]) -> f64; }
pub struct BoardStatistics { pub host: Option<String>, /* … */ }                   // #251
// `RouterJobResourceUsage::{io_read, io_written}` DELETED — #255+#256.

// crates/fr-settings — Task 21.
pub struct ScoringSettings { /* every weight non-optional — #258 */ }
impl RouterSettings { pub fn max_passes(&self) -> MaxPasses; }                      // #140: an `unlimited` flag, not MAX_VALUE
pub enum MaxPasses { Unlimited, Limited(u32) }

// crates/freerouting — Tasks 3, 19, 22.
// `route`: no delete-before-run (#265); `-do` refused AT THE ARGUMENT (#268); the FINAL board
// serialised once after the pipeline (#289).
// `drc`: `--fail-on-violations` (#271, recommendation 5) and `--unit <mm|um|inch|mil>` (#151).
// both forms: exact flag matching and hard failure on an unknown flag or a missing value (#259).
```

---

### Task 0: the gate machinery — the register's status column, the `// fixed:` vocabulary, the four new ids, the generators' `--from-port` mode and `scripts/quality-ab.sh`

**No behavioural change. No golden moves. This task exists so that Task 1 has somewhere to write its evidence.**

**Files:** `docs/java-quirks.md` (the status column, the rules header, the marker table, the four new rows); `scripts/gen-reference.sh`, `scripts/gen-router-reference.sh`, `scripts/gen-batch-reference.sh`, `scripts/gen-cli-reference.sh`, `scripts/gen-drc-reference.sh` (each gains `--from-port`, keeps `--jar`); `scripts/quality-ab.sh` (new); `scripts/differential/run.sh` (the `--against-jar` escape hatch); `crates/fr-core/tests/register.rs` (**new** — the two register tests below live here); `tests/reference/README.md`; `benchmark/candidates.rs.toml` (**committed here**, ruling BP3 — it is currently untracked); `benchmark/scripts/make-java-view.sh` (**new**, ruling BP3); `benchmark/baselines/.gitkeep` (**new** — the directory the rolling baselines live in); `docs/superpowers/plans/2026-09-03-plan-9-post-parity.md` (this file's amendment log).

**Interfaces consumed:** none. **Interfaces produced:** `scripts/quality-ab.sh <task-id>` (the G2 harness, quality **and** `cpu_s`), `gen-*.sh --from-port`, `run.sh <driver> --against-jar`, `benchmark/scripts/make-java-view.sh` (rebuilds the frozen `java-278fe14` results view), the committed `benchmark/candidates.rs.toml`.

**The fix list:** none. This is the plan's bootstrap.

**What it does, item by item.**
1. **The register's status column.** `docs/java-quirks.md`'s three tables gain a `status` column with values `pinned` / `fixed: T<n>` / `totalized` / `candidate` / `keep`. Every existing row is seeded `pinned`, `totalized` or `candidate` from the table it is already in; the ten survey §9.1 rows (#62, #83, #92, #202's three-state stop, #273's order, #281's DTO contract, `normalized_score` as `f32`, #12/#14/#19/#20/#10/#8, the `// Java bug:` markers, §8's KEEP shims) are seeded **`keep`** with §9.1's one-line reason. Recommendation 13, adopted.
2. **The rules header is rewritten.** The line "Do not fix these in the port until parity is proven" is replaced by: *"Parity is proven and is no longer the acceptance test (Plan 9). A row's `status` says whether the port still reproduces it. Fixing a row means editing its status cell and adding a `// fixed: T<n>` comment beside — never deleting — the `// Java bug:` marker."*
3. **The marker vocabulary gains one entry** in the Process-notes table: `// fixed: T<n>` — *"the port no longer reproduces the defect the neighbouring `// Java bug:` marker describes; the line names the Plan 9 task and, in one clause, what the port does instead."* Set by Plan 9.
4. **The four new rows.** Re-read the register's last row (`docs/java-quirks.md:610`/`:889` say the next free id is **#293**) and allocate, per BL6 and recommendation 7: **#293** = R1, the deleted shortest-airline-first ordering; **#294** = R2, the micro-neckdown fanout fallback ignoring the minimum track width; **#295** = I1, the removed every-4th-pass exhaustive tree; **#296** = I2, via inflation between v2.2.4 and v2.3.0. Each row's evidence column cites `benchmark/reports/java-regressions-2026-09.md` and the offending Java commit (`933d2980`, `f31a0c84`), and each is marked **"Java-side fix owed"** — R1 and R2 are the first two upstream PRs on recommendation 8's standing list.
5. **`--from-port` on the five generators.** Each generator keeps its jar path behind `--jar` (for the frozen baseline and for triage) and grows a `--from-port` mode that drives `target/release/freerouting` with the same argv. `gen-router-reference.sh --from-port` must run **the port's own `p6t1` twin**, not a re-implementation, so survey §7.2's property — the reference and the differential describe the same run — survives. Every generator writes `meta.txt` with **the port's git sha, the Plan 9 task at that sha, and the `RouterBudget` in force**; a golden cut before a later fix must be **loudly** invalid, which is what that line is for.
6. **`scripts/quality-ab.sh`.** The G2 harness: 29 stems (8 batch + 13 CLI + 8 DRC), each stem's committed `-mp` cap, one row per stem per metric of the G2 table. It reads the **referee's** DRC for incompletes and violations, never the manifest — the script must fail loudly if handed a manifest number, and its header says why (survey §4.1's 45–63 % disagreement). It writes **`benchmark/baselines/ab/quality-ab-T<n>.tsv`** (committed at task close, ruling BP15) and a copy of its console output to `scripts/differential/out/quality-ab-T<n>.txt`, full, never piped. **Its scope, per rulings BO/BP4, is quality *and* time:**
   - **Quality columns** run with **`RouterBudget::disabled()`** (ruling AI survives the switch: time is out of every quality measurement) and carry **two references per row** — the **port** baseline `benchmark/baselines/ab/quality-ab-T<n-1>.tsv` (what the rules are scored against) and the **jar** column `benchmark/baselines/quality-baseline-java-head.tsv` (context only, no timing, never a gate). The jar tsv lives **outside** the frozen tree; the script must **not** read `tests/reference-frozen/` and its header says so (BL8 as amended by BP1).
   - **A `cpu_s` column per stem**, measured with the budget in its **normal** configuration, as the **median of 3 repeats** with the spread printed beside it, scored against **`benchmark/baselines/stem-times.tsv`** — the previous task's number, never the jar's. The script applies BO's thresholds itself and **exits non-zero** on "equal quality, slower", or prints `ESCALATE: <stem> <ratio>` on **> 2× a stem** or **> 20 % of the corpus median** and exits non-zero, so the escalation cannot be missed by a task that only reads the last line. At task close the script updates `stem-times.tsv` in place, in the same commit as the tsv.
   - Every tsv header carries a **`gate-version`** field (ruling BP8): `g1` from here, `g2` after Task 13, `g3` after Task 19. The script **refuses** to compare two tsvs whose gate-versions differ and says which task must re-cut which column.
7. **`run.sh --against-jar`.** A flag on every converted driver that swaps the committed golden for a live jar run. Not a gate; a triage tool. Its header says so.
8. **`benchmark/candidates.rs.toml`, committed** (ruling BP3). The two-candidate, Java-clone-independent candidates file the milestones use — `java-current` reading the in-repo `binaries/freerouting-current-executable.jar` + `sha_file binaries/freerouting-current.sha` (`278fe14123c4`), `rs-main` reading `../target/release/freerouting`. Its header gains one paragraph saying **why it exists and must not be deleted**: `load_candidates` validates every exec in a file up front, so the 17-candidate `candidates.toml` and the 9-jar `candidates.remote.toml` both fail on a machine that lacks a historical jar or a built Java clone, and this file cannot.
9. **`benchmark/scripts/make-java-view.sh`** (ruling BP3). Rebuilds the frozen **`results/java-278fe14/`** view, because `benchmark/results/` is gitignored and the view is otherwise unrecoverable. Given the source run directory it (a) writes `results/java-278fe14/meta.json` = the source `meta.json` with `run_id` set to `java-278fe14` and the `candidates` array **filtered to the single `java-current` entry** (that filter is the whole mechanism: `bench/compare.py::collect` walks `meta["candidates"]` and takes only the names it was asked for), and (b) symlinks `results/java-278fe14/java-current` at the source run's `java-current` cell directory. It is idempotent, and it **verifies** that the resulting view reports sha `278fe14123c4` over **605** boards, `seeds 1`, `-mp 10`, `timeout 300`, `threads 1`. Its header names `reports/v1.0.0-vs-java-clean.json` as the comparison it must be able to reproduce.

**Tests (named, binding).**
- `scripts/differential/run.sh p6t1` and `p8t1 ci` unchanged (the generators grew a mode; they did not change what they generate in `--jar` mode) — **byte-identical output, proven by re-running both generators in `--jar` mode into a scratch dir and diffing against the committed tree.**
- `crates/fr-core/tests/register.rs::every_java_bug_marker_has_a_register_row` — new: walks `grep -rn '// Java bug:' crates/*/src`, parses the `#nnn` out of each, and asserts the id exists in `docs/java-quirks.md`. Baseline count recorded (**≥ 165**).
- `crates/fr-core/tests/register.rs::a_fixed_row_has_a_fixed_marker` — new: for every register row whose status is `fixed: T<n>`, assert a `// fixed: T<n>` marker exists at some site. **Vacuously true today; it is the gate every later task arms.**

**Evidence / acceptance.** No golden regenerates. G1 green. The two generator-diff runs empty. The register's row count is **296** and its contiguity check (`docs/java-quirks.md`'s own arithmetic note) passes.

**Additionally (ruling BP13) — `quality-ab.sh` is exercised here, not first inside Task 1.** The script is the evidence 22 tasks depend on, and "no golden moves, G1 green" cannot detect a broken one. Task 0 therefore runs a **read-only dry-run**: `scripts/quality-ab.sh --dry-run T0` over the 29 stems against the **current** committed references, plus its `cpu_s` column seeded from `benchmark/results/v1.0.0-rs/`'s per-board `cpu_s` into `benchmark/baselines/stem-times.tsv`. It must produce a complete 29-row tsv with every column populated, a `gate-version: g1` header, zero rows flagged, and **no write anywhere under `tests/`**. The tsv is committed as `benchmark/baselines/ab/quality-ab-T0.tsv` and is the port baseline Task 1 reads. `make-java-view.sh` is also run once here and its verification output pasted.

**Steps:**
- [x] Re-read `docs/java-quirks.md`'s last row and confirm #293 is free; if not, allocate from the real next free id and amend BL6 in place.
- [x] Add the status column to all three tables; seed every row; mark the ten §9.1 rows `keep`.
- [x] Rewrite the rules header; add `// fixed:` to the marker table.
- [x] Write rows #293–#296 with their evidence columns and the "Java-side fix owed" flag.
- [x] Add `--from-port` to the five generators and `meta.txt`'s sha/task/budget line; prove `--jar` mode byte-unchanged.
- [x] Write `scripts/quality-ab.sh` (referee-only quality metrics + the `cpu_s` column, 29 stems, the two references, the `gate-version` header, BO's thresholds).
- [x] Add `run.sh --against-jar`.
- [x] Commit `benchmark/candidates.rs.toml` with its "why this file exists" header; create `benchmark/baselines/` (+ `ab/`).
- [x] Write `benchmark/scripts/make-java-view.sh`; run it; paste its verification output.
- [x] `tests/reference/README.md`: the freeze policy text, pointing at the sibling `tests/reference-frozen/` (Task 1 fills in the directory).
- [x] The two new register tests, in `crates/fr-core/tests/register.rs`.
- [x] The `quality-ab.sh --dry-run T0` smoke run; commit `benchmark/baselines/ab/quality-ab-T0.tsv` and the seeded `benchmark/baselines/stem-times.tsv`.
- [x] G1; commit.

**Commit message:** `chore(plan9): the gate machinery — register status column, // fixed: vocabulary, ids #293-#296, --from-port generators, quality-ab.sh and the bench view script`

---

### Task 1: baseline, determinism and the frozen jar — survey group 1

**This task MUST land before any behavioural fix** (controller direction; survey §10.1 constraint 1). It is the last moment the port is byte-identical to the jar, and the frozen copy is otherwise unrecoverable.

**Files:** `tests/reference-frozen/java-head-2026-09/**` (new, ~34 stem directories copied — a **sibling** of `tests/reference/`, ruling BP1); `tests/reference-frozen/README.md` (new); `tests/reference/README.md`; `benchmark/baselines/quality-baseline-java-head.tsv` (new — the jar column, written **outside** the frozen tree); `benchmark/baselines/stem-times.tsv` (the port timing baseline, seeded at Task 0, re-stamped here); `benchmark/candidates.toml`; `crates/fr-router/src/pipeline/stop.rs` (**`RouterBudget` is declared here at `:493`; `impl Default` at `:522`** — ruling BP5); `crates/fr-core/src/ctx.rs` (the holder at `:42`/`:56`); `crates/fr-settings/src/router_settings.rs` (the new `router.opt_changed_area_ms` field); `crates/freerouting/src/commands/route.rs`; `crates/fr-router/src/tightener/mod.rs` (the pull-tight budget's reader); `crates/fr-router/src/pipeline/fanout.rs` (`parse_timespan_seconds`); `crates/fr-router/src/autoroute/connection_router.rs` (`retry_connection_necked`); `crates/fr-router/tests/{batch_autorouter.rs,fanout.rs}`; `crates/freerouting/tests/cli_e2e.rs`; `scripts/gen-cli-reference.sh` (the two-run identity mode).

**Interfaces consumed:** `fr_core::ctx::RouterBudget` (Plan 8 Task 0), `fr_router::pipeline::RouterStop` (Plan 7), `scripts/quality-ab.sh` and the five `--from-port` generators (Task 0).
**Interfaces produced:** `impl Default for RouterBudget` with `opt_changed_area_ms = 0` (in `pipeline/stop.rs`); the `router.opt_changed_area_ms` setting; `benchmark/baselines/quality-baseline-java-head.tsv`; the port-side two-run identity check.

**The fix list (3 rows).**

| # | mechanism (one line) | fix sketch |
|---|---|---|
| **#234** | `optChangedArea`'s 1000 ms `TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP` is a `static final int` with a constant initialiser, so `javac` **inlines** it and no flag or reflection switches it off; it abandons the pull-tight mid-way on wall-clock, and the jar is measurably non-reproducible from pass 3 (score 841.0115 at `-mp 3` vs 835.88336 at `-mp 20`, three runs each). **The port's CLI runs `RouterBudget::default()`, i.e. Java's live 1000 ms** — a machine-speed dependency in the program whose output is about to become the reference. | **Make the CLI deterministic**: default `opt_changed_area_ms` to **0** (Java's own "off" value) and expose it as a real setting `router.opt_changed_area_ms`. If a bound is ever wanted, bound the pull-tight **by work, not by clock**. Ruling AI's "time out of every measurement" survives unchanged. |
| **#208** | `retryConnectionNecked` **re-uses the `TimeLimit` the failed first attempt already spent** — and the connections the retry exists for are exactly the ones where the first attempt ran long, so the remaining budget is smallest precisely where the retry is wanted. RED/GREEN already measured: **81 ms of a 96 ms call**. | Build a **fresh** `TimeLimit`; the parameter then disappears. The register's alternative — "say in a comment that it is deliberately time-boxed" — is **not** a fix. Measure with the connection time limit **disabled**, then separately prove the retry is time-boxed. |
| **#224** | `FanoutSettings.timeout`'s two **documented** examples, `"5m"` and `"300s"`, both parse to `null` — `parseTimespanString` splits on `':'` and builds `PT5mS` / `PT300sS`, which `Duration.parse` rejects, and the exception is swallowed, so the fanout stage silently runs with **no** timeout. `optimizer.timeout` and `jobTimeoutString` use the same method. | Accept the unit suffixes the javadoc promises (`5m`, `300s`, `1h30m`), and **reject an unparseable timeout loudly** rather than running unbounded. Pairs with the in-pass deadline observation already landed at `c3a7ee0`. |

**Note #208 is placed here deliberately.** The survey's §10.3 group table omits it — it is the third FIX row of §4.11 and no group claims it. It belongs with #234 and #224: all three are the clock, all three change how much work a run does, and all three want the same single regeneration. Recorded as an ambiguity resolved (Task 25's report carries the line).

**The non-fix mechanics, in order.**
1. **Freeze the jar copies FIRST, in their own commit, before #234 touches anything — into a SIBLING directory** (ruling BP1, fixing a copy-into-itself). The target is `tests/reference-frozen/java-head-2026-09/`, **never** a child of `tests/reference/`:
   ```
   mkdir -p tests/reference-frozen
   rsync -a tests/reference/ tests/reference-frozen/java-head-2026-09/
   ```
   (`rsync -a src/ dst/` copies the *contents* of `tests/reference/`, and because the destination is a **sibling** rather than a child there is no self-recursion to exclude. The earlier draft's recursive copy of `tests/reference/` into a directory **inside itself** could not express that on either BSD or GNU: its parenthetical "minus the new directory itself" named no mechanism, and the copy either recurses or errors.) **Verify before going further**: `diff -r tests/reference tests/reference-frozen/java-head-2026-09` is empty, and the file and byte counts match on both sides. `tests/reference-frozen/README.md` (new) carries the freeze paragraph: *"This directory is the historical baseline the port was proven byte-identical to at `c467758`, generated by the clone's HEAD jar. It is **never again an assertion** — no test and no script reads it (BL8 as amended by BP1); the numbers derived from it live in `benchmark/baselines/`. It exists so that a future reader can answer 'what did the jar do here?' after the differential drivers are gone."* `tests/reference/README.md` gains a one-line pointer to it. Record the byte count and file count in the commit message.
2. **`benchmark/baselines/quality-baseline-java-head.tsv`, written BEFORE the freeze is made read-only** (ruling BP1, fixing a write-into-a-chmodded-tree). Order within the freeze commit is fixed and is the whole point:
   1. `rsync` the tree in and verify it;
   2. derive the G2 **jar column** from it — the referee's DRC over each frozen `route.ses`/`batch.ses`, plus the score/length/via/bend columns — and write it to **`benchmark/baselines/quality-baseline-java-head.tsv`**, *outside* the frozen tree, with a header saying it is **numbers, quality only, context and never a gate**, and that it carries **no timing column** (BO forbids the jar as the timing reference);
   3. **only then** `chmod -R a-w tests/reference-frozen/java-head-2026-09/`.
   Nothing is ever written inside the frozen directory after step 3, and nothing reads it after step 2.
3. **`benchmark/candidates.toml`, and the correct provenance** (ruling BP2). `[candidates.rs-main]` is live and **has been since `ecc0abf`, the import** — `git log -- benchmark/candidates.toml` shows exactly that one commit. **`c467758` changed four documentation files and no bench code at all**, despite its subject line; do not infer from it that any candidate path was fixed. So: **verify `candidates.toml` itself** — every `exec` resolves on this machine, and `java-current`'s does so only via the read-only Java clone's build output — **delete the stale `# NOTE: uncomment once the fork builds`**, and record in the commit message that the milestones do **not** use this file (they use `benchmark/candidates.rs.toml`, committed at Task 0, per ruling BP3). **No `bench run` happens in this task.** The pre-fix position M1 is compared against is the **existing `v1.0.0-rs` run of record** (`reports/v1.0.0-vs-java-clean.json`): `c467758` and `eff706a` are documentation-only commits on top of v1.0.0, so the binary that produced it *is* this branch's base, and a fourth full bench would measure the same thing at the cost of two hours.
4. **The two-run identity check.** After #234 lands, add the port-side analogue of `gen-cli-reference.sh --verify-hash-modes`: **run every stem twice and require byte-identical output.** There is no `-XX:hashCode` axis on the Rust side; two runs on one machine plus one run on CI is the equivalent assertion. It becomes part of G1 for the rest of the plan.
5. **~~Regenerate G, R, B, C, D from the port~~ — CLOSED by ruling BT: no regeneration at Task 1.** Task 1 executed the regeneration, measured it completely, and reverted it. **Measured zero golden movement**: none of #234, #208 or #224 moves a single routed byte on any stem, from five independent directions (both test lanes green throughout; router JSONL 0/6 rows and `roundtrip.dsn`/`unrouted.ses` 0 files moved; `batch.ses` and `route.ses` moved by *exactly two lines each* and those two lines are quirk #92's `hostCad`→`host_cad`; all 13 manifests unchanged in score, length, vias, bends and incompletes; corpus-median `cpu_s` ratio **1.000**). The reason is structural — every parity driver already ran `RouterBudget::disabled()`, so B and R could not move, and the CLI lane sits on stems where the 1000 ms limit never fires (the register's own "0 trips on all eight whole-board stems"). #208 and #224 are unreachable from the corpus entirely: no stem sets a neck width or any timeout. **What remains of step 5 is therefore a pure lane switch, and ruling BT defers it — see the amended G1 cadence above.** The one standalone piece landed: `gen-cli-reference.sh`'s budget note, whose `meta.txt` port-lane line now carries `opt_changed_area_ms = 0` beside the sha and the task.

**Tests (named, binding).**
- `crates/fr-core/tests/ctx.rs::the_default_router_budget_disables_the_opt_changed_area_clock` — fails before (`default()` is 1000 ms), passes after.
- `crates/freerouting/tests/cli_e2e.rs::two_runs_of_every_ci_stem_are_byte_identical` — the two-run identity check, four CI stems.
- `crates/fr-settings/tests/router_settings.rs::opt_changed_area_ms_is_a_settable_field` — `--router.opt_changed_area_ms=250` reaches `RouterBudget`.
- `crates/fr-router/tests/batch_autorouter.rs::the_necked_retry_gets_a_fresh_time_limit` — asserts the retry's budget is the full per-connection limit, not the remainder; fails before with the measured 81/96 ms shape.
- `crates/fr-router/tests/fanout.rs::the_documented_timeout_spellings_parse` — `"5m"` → 300 s, `"300s"` → 300 s, `"1h30m"` → 5400 s; and `parse_timespan_seconds("banana")` is an **error**, not `None`-then-unbounded.
- `crates/fr-router/tests/fanout.rs::an_unparseable_fanout_timeout_is_refused` — the run fails at the setting with the string named.

**Evidence / acceptance.**
- The freeze commit: file count and byte count recorded; `diff -r` empty **before** the `chmod`; the `chmod` verified **after** the tsv was written outside the tree; **nothing reads the directory** — `grep -rn "reference-frozen" crates/ scripts/ benchmark/` returns **only** the two READMEs' prose (ruling BP1; `quality-ab.sh` reads `benchmark/baselines/quality-baseline-java-head.tsv` and never the frozen tree).
- ~~**G, R, B, C, D all regenerate**, deliberately and once, with the `goldens moved:` line *"#234 … batch SES bytes move on 8/8 stems, router JSONL on 6/6 rows, CLI on 13/13 stems."*~~ **Superseded by ruling BT — see step 5.** That prediction was measured false: **`goldens moved: NONE`**, on every family, in both lanes. The direction check still ran and is the record — total trace length must **fall or hold** on every stem (a fully-run pull-tight cannot lengthen a trace), and it **held on 13/13 with a rise on none**, alongside every score, via count and bend count. A family now regenerates only when a fix moves it (the amended G1 cadence).
- G2: incompletes must not rise on any of the 29 stems; violations 0 on every stem. Scored against **`benchmark/baselines/ab/quality-ab-T0.tsv`** (the port baseline Task 0 cut), with the jar column beside it as context.
- **Timing (rulings BO/BP4) — Task 1 is one of the four tasks named in advance as likely to escalate.** #234 stops the pull-tight being abandoned on wall-clock, so **every multi-pass stem does strictly more work** — *predicted*, and **measured false on this corpus**: the limit takes 0 trips on all eight whole-board stems in their reference configuration, so the corpus-median `cpu_s` ratio came out at **1.000** and no stem approached a threshold. The gate below is unchanged and still binds for later tasks: `> 2×` on any stem or `> 20 %` on the corpus median **escalates to the controller for an explicit keep/rework ruling before the task closes** — pre-authorised here, so the escalation is a message, not a re-plan. `benchmark/baselines/stem-times.tsv` is re-stamped at task close with this task's medians, and that is the number Task 2 is scored against.
- **No `bench run` in this task** (ruling BP3). M1's pre-fix reference is the existing `v1.0.0-rs` run of record.

**Steps:**
- [x] Commit 1 (`35f9078`): freeze into the sibling `tests/reference-frozen/java-head-2026-09/` (rsync, `diff -r` verified), **then** write `benchmark/baselines/quality-baseline-java-head.tsv`, **then** `chmod -R a-w`; the two READMEs and the counts. **No code change in this commit.**
- [x] Commit 2 (`2b84fc2`): `candidates.toml` verified and tidied, with the corrected `ecc0abf` provenance in the message.
- [x] Commit 3 (`a8892e8`): #234 — `impl Default for RouterBudget` (in `pipeline/stop.rs`) to 0, the new setting, the CLI wiring, the two directed tests.
- [x] Commit 4 (`b896507`): #208 — the fresh `TimeLimit`, its directed test.
- [x] Commit 5 (`e86c109`): #224 — the timespan grammar and the loud refusal, its two directed tests.
- [x] ~~Commit 6: regenerate G/R/B/C/D `--from-port`~~ — **closed by ruling BT with no regeneration**; the direction check ran and `goldens moved: NONE`. The one standalone piece landed as `4c59c86` (the CLI generator's budget note, whose port-lane `meta.txt` carries `opt_changed_area_ms = 0`), and the closure itself as `ea10866`.
- [x] Commit 7 (`adf8358`): the two-run identity check, wired into G1 for every later task.
- [x] Commit 8 (`80b7e24`): `quality-ab.sh T1` — commit `benchmark/baselines/ab/quality-ab-T1.tsv` and the re-stamped `benchmark/baselines/stem-times.tsv`; escalate the `cpu_s` ratio if it trips BO's thresholds. **Corpus-median ratio 1.000; nothing to escalate.**
- [x] Register: #234, #208, #224 → `fixed: T1`, with `// fixed: T1 (#234)` / `(#208)` / `(#224)` markers beside their `// Java bug:` lines (A17: the id is required and the test counts one per site).

**Commit message (final commit of the task).** The plan proposed one summary commit, `feat(determinism): the clock leaves the router — … ; goldens re-cut from the port`. **Not written, and its second clause is why**: BL4's cadence made the three fixes three commits (`a8892e8`, `b896507`, `e86c109`), each carrying its own evidence, and no goldens were re-cut (ruling BT). The task's closing commits are `adf8358` (the two-run identity check), `80b7e24` (`quality-ab-T1.tsv` and the re-stamped `stem-times.tsv`) and `ea10866` (BT's closure).

---

### Task 2: the two measured regressions — survey group 2 · **MILESTONE M1**

**The strongest quality evidence in the catalogue**, and the reason it goes second: R1 and R2 move the baseline every later A/B is taken against, so any measurement taken before them measures the regression rather than the fix (survey §10.1 constraint 2). Neither needs a new fixture, a new harness or a discovery phase.

**Files:** `crates/fr-router/src/pipeline/airline.rs` (**three of the roster's five** `// not ported:` lines — `:23` `calculateItemDistance`, `:24` `calculateMinDistance`, `:25` `getItemReferencePoint` — become ported methods; `:21` `nearestPointOnTrace` and `:22` `findClosestPointsBetweenTraces` stay unported); `crates/fr-router/src/pipeline/batch_autorouter.rs` (**both** `autoroute_items` `:702` and `autoroute_items_with_handled` `:714` — the sort lands in the latter, which is the p7t1 differential seam, and the wrapper is untouched; ruling BP5); `crates/fr-router/src/pipeline/pass_runner.rs` (`:168`, the consumer); `crates/fr-router/src/autoroute/path/inserter.rs` (`insert_fanout_micro_neckdown`, `:488-…`, the candidate list at `:509-…`, called from `:394`); `crates/fr-board/src/rules/board_rules.rs` (`get_min_trace_half_width`, `:138` — **the reader**, not `crates/fr-board/src/board/mod.rs:1787`); `crates/fr-router/tests/{batch_autorouter.rs,inserter.rs,fanout.rs}`; `crates/fr-router/README.md` (the roster's re-stated greps); `docs/java-quirks.md`; `benchmark/reports/java-regressions-2026-09.md` (a "status: fixed in the port at Plan 9 Task 2" header line).

**Interfaces consumed:** `fr_board::BoardRules::get_min_trace_half_width` (Plan 2), `fr_board::Board::get_items` (Plan 2), `fr_router::pipeline::pass_runner` (Plan 7).
**Interfaces produced:** `fr_router::pipeline::calculate_item_distance`; `autoroute_items_with_handled` (and therefore its `autoroute_items` wrapper) returning a **sorted** list — both keep their existing signatures.

**The fix list (4 rows: 2 fixes, 2 investigations).**

| # | mechanism | fix sketch |
|---|---|---|
| **R1 = #293** | **The shortest-airline-first ordering of the items to route was deleted.** Commit `933d2980` ("Remove useSlowAlgorithm parameter from autorouter", 2026-01-14, v2.2.0) removed `autoroute_item_list.sort(Comparator.comparingDouble(this::calculateItemDistance))` with the note "Disabled in v2.3 because it negatively impacts convergence compared to v1.9 (natural order)". **The data contradicts the note.** Fully-connected drops 0.81 → 0.75 at v2.2.0 and never recovers; 17 probe boards v2.1.0 completes are left unrouted by every 2.2.x. Restoring the sort alone at HEAD returns connectivity to **0.76 → 0.82** and is *slightly faster*; neutral within noise on medium/large. The port transcribed HEAD faithfully: `autoroute_items` returns the list in `board.itemList` **descending-id** order (quirk #63) and nothing sorts it. `calculateItemDistance` (`AutorouteAirlineCalculator.java:162-177`) still exists in Java and has **no caller** — caller-less precisely because this sort was deleted. | Port the three caller-less `AutorouteAirlineCalculator` methods (`airline.rs`'s roster names them at its `:23`, `:24` and `:25` — Java `:162-177`, `:179-202`, `:204-213`; its other two `// not ported:` lines, `nearestPointOnTrace` and `findClosestPointsBetweenTraces`, are **not** resurrected) and **sort the work list ascending by `calculateItemDistance`** inside `autoroute_items_with_handled`, before the pass runner walks it. **Unconditionally** — no tier measured worse with it — with the tie order left as the **descending-id walk**, so the sort is a *stable refinement* of today's order and not a second reordering. The three `// not ported:` lines become ported methods and **their greps in the roster must be re-stated in the same commit**. |
| **R2 = #294** | **The micro-neckdown fanout fallback ignores the board's minimum track width.** Commit `f31a0c84` (2026-05-19, v2.3.0 — the port carries it) retries a failed 2-point fanout insertion at the pin's neckdown half-width, then `3/4`, `3/5` and `1/2` of the class half-width, "keeping the same clearance class", checking nothing against the design rule. The loop guard at `FoundConnectionInserter.java:473` is `candidateHalfWidth <= 0 \|\| >= baseHalfWidth`, **and nothing else**. On any board whose net-class width **equals** its minimum width — very common — it emits sub-minimum traces: `track_width` violations on **31/146** small boards, DRC-clean **0.94 → 0.73** (small) and **0.93 → 0.40** (large). Because a clean-pass metric weighs a violation like an unrouted net, the fallback converts "one net open" into "board fails DRC", and on the small tier it recovers **no** connectivity at all. Its benefit is real only on large boards (connected 0.17 → 0.33). | **Guard, do not revert.** Skip every candidate half width **below the board's minimum**: `fr_board::BoardRules::get_min_trace_half_width()` (`BoardRules.java:37` — the minimum over the *declared net-class widths*) — **not** `Board::get_min_trace_half_width`, which is a running minimum over the traces already inserted and would ratchet itself down. When the class width already **is** the minimum, the whole fallback is skipped and the connection **fails honestly**. |
| **I1 = #295** | The same commit `933d2980` also removed the exhaustive ("slow") search tree that ran on every 4th pass (`useSlowAlgorithm = passNo % 4 == 0` → `false`). **The report does not implicate it** — the sort ablation alone recovers v2.1.0's connectivity — but the flag's remains are in both trees (`BoardRules.java:48, :395, :404`, no reader; the port's ported twin at `crates/fr-board/src/rules/board_rules.rs:61, :514, :519`), and the "slow" tree is the *base* `ShapeSearchTree`, i.e. the one whose `completeShape` **keeps** the room the 90-degree override drops (**#159**). So an every-4th-pass slow tree was, among other things, a periodic workaround for #159. | **Investigation, not a fix.** Task 2 records the question and the two dead accessors; the **measurement runs after #159 lands (Task 8)** and its answer is written into Task 8's report: if a periodic exhaustive tree still buys nothing, delete the flag in both trees. **Do not restore it blind** — it is a 4× cost on every 4th pass with no measured benefit. |
| **I2 = #296** | **Via inflation.** Between v2.2.4 and v2.3.0 via usage roughly **doubles** (0.37× → 0.71–0.94× of the human reference count) alongside a ~25 % slowdown — the report reads it as the "recovery" work after R1's ordering regression trying harder with more vias. Not root-caused. | **Re-measure after R1 lands, at M1.** If the inflation is the ordering regression's downstream compensation, restoring the sort deflates it on its own. If it survives R1, bisect v2.2.4→v2.3.0 with the suite — and read it together with **#172** (a 10× via discount on pure-SMD nets, Task 18), which is plausibly the same phenomenon from the other end. Via count is a first-class G2 metric, so this is measured either way. |

**Tests (named, binding).**
- `crates/fr-router/tests/batch_autorouter.rs::the_work_list_is_sorted_by_airline_distance` — a three-item board whose descending-id order and whose airline order disagree; asserts the routed order is ascending by distance. **Fails before** (the list comes back in descending id).
- `crates/fr-router/tests/batch_autorouter.rs::equal_airline_distances_keep_the_descending_id_tie_order` — the stability claim, asserted explicitly so a later sort change cannot silently reorder ties.
- `crates/fr-router/tests/airline.rs::calculate_item_distance_matches_the_java_formula` — the three ported methods against hand-computed values on a pin/via/trace triple (the Java code is `:162-213`; the expectations are hand-computed and reviewed, not read from the jar — recommendation 9).
- `crates/fr-router/tests/inserter.rs::the_micro_neckdown_fallback_never_goes_below_the_rules_minimum` — a board whose class width **equals** the rules minimum: every candidate is rejected and the connection fails. **Fails before** (a sub-minimum trace is inserted).
- `crates/fr-router/tests/inserter.rs::the_fallback_still_necks_down_when_the_class_is_above_the_minimum` — the large-board benefit is preserved; the 3/4 candidate is still taken when it clears the minimum.
- `crates/fr-router/tests/inserter.rs::the_guard_reads_the_rules_minimum_not_the_running_board_minimum` — inserts a narrow trace first, then asserts the guard does **not** ratchet down. This is the test that pins the survey's one-sentence correction.

**Evidence / acceptance.**
- **G1** green; the four directed tests fail-before/pass-after with output pasted.
- **B, R, C regenerate on every routed stem** — this is the largest single golden churn in the plan and it is expected. `goldens moved:` reads: *"R1 (#293) — the work list is now airline-sorted, so every routed stem's connection order changes; R2 (#294) — sub-minimum fanout traces are no longer inserted, so stems with SMD fanout lose them."* Direction check: **incomplete count must fall or hold on all 29 G2 stems**, and **`track_width`-class violations must go to zero** on every stem that had them.
- **G2**: incompletes fall on at least one stem; violations 0 everywhere.
- **MILESTONE M1** — the G3 block **exactly as written**, with `--run-id plan9-m1`, compared `--runs java-278fe14,plan9-m1`; the compare lands at `benchmark/reports/plan9-m1-vs-java-278fe14.{json,md}` (`git add -f`) and `remote-run.sh`'s console output at `benchmark/baselines/plan9-m1.log`. The workbench binary is built at this task's head commit and its sha is pasted beside the `meta.json` sha it must equal. **Acceptance, from the report's ablation numbers, read off the compare's per-board rows for the report's D3 subsets** (there is no `--tier small`): **D3-small clean pass ≥ 0.77**, **connected ≥ 0.79**, **DRC-clean ≥ 0.95**; **D3-large clean pass ≥ 0.30**. `overall.verdict` `better` or `same`, **zero hard losses**, against the frozen `java-278fe14`. Plus the **corpus-median `cpu_s` ratio against the `v1.0.0-rs` run of record** (BO/BP4).
- **If M1 misses a threshold** (ruling BP12) — the task does **not** silently proceed and does **not** stall. The ablation was measured on the jar and the port is a port, so a gap is a real difference and is treated as one: the task's deliverable becomes a **root-cause report** — which of R1/R2 under-performs, bisected by re-running the milestone with that fix reverted (rs-only, one extra run-id, controller-authorised) or by the stem A/B where the corpus is too coarse — and the **controller adjudicates**: revert, rework, or accept with the gap recorded. M1 is then **re-run once** at the adjudicated commit. A missed threshold is an escalation with a named next step, never "the task is not done" with no exit.
- **The neckdown column is owed to Task 14** (ruling BP15): this task's `benchmark/baselines/ab/quality-ab-T2.tsv` carries a `neckdown_below_class_width` count per stem **before and after R2**, committed, so Task 14's joint table reads it rather than reconstructing it.
- **I2's column recorded**: via count per board and per D3 subset at M1 vs the `v1.0.0-rs` run of record, written into `benchmark/reports/java-regressions-2026-09.md` as a status appendix.
- Drivers touched: `p6t1`, `p7t1`, `p7t2`, `p7t5`, `p7t9`, `p8t1` all re-golden here; none retires.

**Steps:**
- [x] Commit 1 (`e4ea4c1`): R1 — port the three airline methods, re-state the roster greps, sort ascending with the descending-id tie order; three directed tests.
- [x] Commit 2 (`fb1ff6c`): R2 — the rules-minimum guard on the candidate list; three directed tests.
- [x] Commit 3 (`ebe38e7`): I1/I2 — register rows #295/#296 written with their open questions and their owners (I1 → Task 8, I2 → M1 + Task 18); **no code**.
- [x] Commit 4 (`bd296d7`): regenerate B/C `--from-port` (the #92 lane switch and the `stats.rs` `hostCad` migration ruling BT pre-resolved); the direction checks; the `goldens moved:` line. Task 2's escalation and its review round landed as `27e1b75` + `5152291`.
- [x] Commit 5: **M1** — run at `5152291`, results committed at **`a0eb84e`**, adjudicated by **ruling BV: ACCEPT-WITH-GAP** (BP12's third branch). The closure, the artefact table and the accept wave are below.
- [x] Register: #293 → `fixed: T2, accepted: M1 (BV)`, #294 → same, #295/#296 → `candidate` with their owners; #296 carries M1's via measurement.

**M1 closure (ruling BV, at `a0eb84e`).** The bench ran on `workbench` at the adjudicated head
`5152291`, `--candidates rs-main` only, against the frozen `java-278fe14` view, 605 `pcbench`
boards, `seeds 1`, `-mp 10`, `timeout 300`, `threads 1`, `jobs 12` — ruling BN's recipe exactly.

* **Corpus clean pass 0.375 → 0.550** (+17.5 points).
* **D3-small**: clean 0.587 → **0.755** (bar 0.77), connected 0.755 → **0.783** (bar 0.79),
  DRC-clean 0.727 → **0.958** (bar 0.95 — **hit**). **D3-large**: clean 0.178 → **0.292**
  (bar 0.30). Three of the four thresholds are missed by **1–3 boards**, and the port lands within
  1–3 boards of the jar's own post-fix ablation on every one of them.
* D3-large connected 0.371 → 0.322 is substantially the legality trade — large DRC-clean goes
  0.386 → **0.812** across the same boards.
* **Corpus-median `cpu_s` ratio against the `v1.0.0-rs` run of record: 1.000** (BO/BP4); corpus
  total `cpu_s` 15 245.2 s → 15 193.3 s, i.e. 0.9966. No drift, and no timing escalation.
* **Ruling BV** accepts with the residual bar gap attributed to known post-parity deltas
  (tie-break and friends), files the large-tier R1 attribution question to **Task 18** as a policy
  row (net-count-gated sort, A/B'd at M3), and does **not** spend BP12's one re-run.

**The M1 artefact table (Task 2's report §SF3), verified at the accept wave:**

| artefact owed | state |
|---|---|
| the frozen-view compare, `--runs java-278fe14,plan9-m1`, `git add -f` | **committed at `a0eb84e`** as `benchmark/reports/plan9-m1-vs-java.json` — the name is shorter than SF3's `plan9-m1-vs-java-278fe14.json`, and that is the file that exists |
| its rendered form, same run pair, `git add -f` | **committed at `a0eb84e`** as `benchmark/reports/plan9-m1-vs-java.md`. The compare also wrote a `.html`, which is **not** committed (`benchmark/.gitignore`'s `reports/*.html`) |
| `remote-run.sh`'s console output, in full | **not under `benchmark/baselines/`, and it cannot be**: `remote-run.sh:256` rsyncs `results/<run-id>/` back and nothing beside it, so the console log stays on the workbench at `<remote-dir>/results/plan9-m1.remote.log`. What is local is the **full run tree**, `benchmark/results/plan9-m1/` — 11 476 files, `meta.json` plus per-board `stdout.log`, `stderr.log`, `referee.log`, `metrics.json`, `out.ses` and the routed KiCad files — gitignored by `benchmark/.gitignore`'s `results/`, exactly as `v1.0.0-rs` is. **Recorded as located, not as missing** |
| the workbench sha check | `benchmark/results/plan9-m1/meta.json`'s `candidates[0].sha` is **`5152291b085a`** — the adjudicated head commit, which is what it must equal. The ledger's build line records the fresh binary sha **`2e05308b`** built at that checkout, after the untracked `candidates.rs.toml` that produced a false `REMOTE-READY` was cleared |

**The accept wave (ruling BV), landed on top:** the 14 release-lane jar-parity literals re-cut as
port-regression pins with provenance at each site; `p8t1` and `p8t7` rungs (a)/(b) converted to
port-golden comparison with the jar arm behind `--against-jar` and the R1/R2 cause recorded in
`scripts/differential/README.md`'s "Converted drivers" table; G2's "incompletes must not rise"
gains ruling BU(b)'s BP12 arm — *on an escalated task the M-bench is the gate, G2 records* — in
both this plan's G2 section and `scripts/quality-ab.sh`'s header; the register rows above; and I2
(#296) answered from the corpus: **the via inflation survives R1 + R2** (15 539 → 16 129 vias,
+3.80 % against the port's own pre-fix position), so its bisect branch is now due. `stem-times.tsv`
needs nothing further — Task 3's re-cut plus the S4 correction stand.

**Commit message (final commit of the task):** `feat(router): repair the two measured Java regressions — R1 (#293) airline-first ordering restored, R2 (#294) micro-neckdown floored at the rules minimum; M1 bench recorded`

---

### Task 3: output integrity — survey group 3

**One code path.** `commands/route.rs`'s output half loses the user's result in three different ways, and they must be fixed together: delete-before-run, discard-the-return, and write-the-wrong-board (survey §10.2).

**Files:** `crates/freerouting/src/commands/route.rs` (`delete_existing_output`, `_accepted`, `set_job_output`, `write_cli_output_if_available`, step 12b's pre-routing snapshot, `resolved_output_format`); `crates/fr-core/src/{job.rs,file_details.rs}` (`try_to_set_output_file`'s return, `set_data`'s re-sniff); `crates/freerouting/tests/cli_e2e.rs`; `tests/reference/cli-kicad-ecc83-json/*`, `tests/reference/cli-kicad-complex-hierarchy-json/*`.

**Interfaces consumed:** `fr_core::{RoutingJob, BoardFileDetails, FileFormat}` (Plan 8 Task 1), `fr_dsn::kicad::write` (Plan 8 Task 10).
**Interfaces produced:** `route` refuses an unsupported `-do` extension **at the argument**; the KiCad-JSON output holds the **final** board.

**The fix list (3 rows).**

| # | mechanism | fix sketch |
|---|---|---|
| **#289** | **`-do out.json` writes the board as it was BEFORE routing.** `setJobOutput` is a board-updated listener *and* runs once after the pipeline; the first write re-sniffs its own bytes to `KICAD_DESIGN_JSON`, so every later write is a no-op and the file keeps the loaded board. Measured identical at `-mp 1`, `-mp 2`, `-mp 8`, and with the router off. Java: `RoutingJobSchedulerActionThread.java:100, :168, :259-295`; `BoardFileDetails.java:105-119`. Port: `commands/route.rs` step 12b takes the pre-routing snapshot **deliberately**, to match. | Write the **final** board: serialise **once, after the pipeline**, and stop re-sniffing (`setData(bytes, format)` keeps the format it was given). **Delete** the pre-routing snapshot and `resolved_output_format`'s shared-snapshot dance. |
| **#268** | `tryToSetOutputFile`'s return is discarded, so `-do out.dsn`/`.scr` **writes a 0-byte file and exits 1** — over the previous result #265 has already deleted — while `-do out.txt` silently receives SES bytes and exits 0. Java: `Freerouting.java:123, :196-213`; `RoutingJob.java:377-397`. | Test the return value and **refuse the run at the argument**, naming the accepted formats. **Never write a zero-byte file.** |
| **#265** | The desired output file is **deleted before the run starts** — before input validation, before the board load, before the router. A run that then fails, hangs or is killed has destroyed the previous result and written nothing. `File.delete()` also unlinks an empty directory. Java: `Freerouting.java:116-121`. Port: `commands/route.rs::delete_existing_output`. | **Delete nothing.** `std::fs::write` truncates; a failed run must leave the previous result on disk. |

**Tests (named, binding).**
- `crates/freerouting/tests/cli_e2e.rs::do_out_json_writes_the_routed_board` — the inverse of Plan 8's `do_out_json_writes_the_pre_routing_board`, which is **deleted in this commit** and whose deletion is named in the message. Asserts the JSON's wire count matches the SES's.
- `crates/freerouting/tests/cli_e2e.rs::an_unsupported_output_extension_is_refused_at_the_argument` — `-do out.dsn` exits 1 **before** the board loads, with the accepted formats in the message, and **no file is created**.
- `crates/freerouting/tests/cli_e2e.rs::a_failed_run_leaves_the_previous_result_on_disk` — write a sentinel file at the `-do` path, run with a malformed input, assert the sentinel survives byte-for-byte.
- `crates/freerouting/tests/cli_e2e.rs::an_empty_output_directory_is_not_unlinked` — the `File.delete()` directory case.
- `crates/fr-core/tests/file_details.rs::set_data_keeps_the_format_it_was_given` — the re-sniff is gone.

**Evidence / acceptance.** G1 green. ~~**C moves on the 2 KiCad stems** (`cli-kicad-ecc83-json`, `cli-kicad-complex-hierarchy-json`): their `route.json` now holds a routed board. `goldens moved:` names #289 and the direction (wire count rises from 0 to the SES's count).~~ **Measured false — see amendment A22.** No `tests/reference/cli-*` stem writes a `.json` output: all thirteen write `<OUT>/route.ses`, and the two "KiCad JSON" stems are named for their *input* format. **`goldens moved: NONE`**, and no family regenerates (ruling BT, applied as written). The #92 lane switch and BT's pre-resolved `stats.rs` fixture migration are **Task 2's**, carried with its B/C regeneration at `bd296d7` — BT lands them per family, the first time that family moves for cause, and Task 3 moves none. `cli_e2e` and `p8t7`'s rung (c) are rewritten instead — the rung is now an **XDIFF** that measures both programs. Plan 8 hand-off §3's divergence rows **10** and the `-do out.json` half of row **4** are struck and replaced with a "fixed in Plan 9 Task 3" line — Task 25 carries the edit. G2 unaffected, and **asserted**: 377 quality cells compared against T1's tsv, 0 moved.

**Steps:**
- [x] Commit 1: #265 — delete `delete_existing_output` and its call; two directed tests. — `94cb73d`.
- [x] Commit 2: #268 — the return is tested, **and so is the resolved format's writability** (`true` alone only means the extension was recognised; `-do out.dsn` answers `true` and is still unwritable), the refusal moves to the argument; one directed test. — `5675754`.
- [x] Commit 3: #289 — one serialisation after the pipeline, the snapshot and the re-sniff deleted; two directed tests, **four** Plan 8 tests deleted by name across the three commits (the other three pin the behaviour #265 and #268 remove). — `ad7208b`.
- [x] ~~Commit 4: re-cut the two KiCad C-stems~~ — **nothing to re-cut (A22)**; Commit 4 is the measurement close: `quality-ab-T3.tsv`, `goldens moved: NONE` with the two commands that prove it. — `d59a729`.
- [x] Register: #289, #268, #265 → `fixed: T3`.

**Commit message:** `fix(cli): the output half stops losing the user's result — #265 no delete-before-run, #268 refuse at the argument, #289 write the final board`

---

### Task 4: DSN read integrity — scopes, scale, size — survey group 4

**Ordering inside the task is fixed** (survey §10.1): **#95 → #90** (#90's caller-side desync is inside the scope #95 repairs), and **#93 + #89 + #94 in ONE commit** (#93 alone changes the board size, #94 alone changes which boards degenerate, #89 alone converts a silent `Infinity` into a loud error).

**Files:** `crates/fr-dsn/src/parser/structure.rs`, `.../autoroute_settings.rs`, `.../dsn_file.rs` (`read_integer_scope`, `skip_scope`, `read_scope`), `.../network.rs` (`insert_component`), `.../geometry.rs` (the circle bounds); `crates/fr-dsn/src/rules_reader.rs`; `crates/fr-dsn/src/lib.rs` (`BoardReadResult`); `crates/fr-dsn/src/lexer/scanner.rs` (`DsnScanner::new` — **#86's input ceiling only. `scanner.rs` is edited again at Task 22 (#84/#85: the skip/stop sets and the number grammar); the two edits are in different functions, Task 4 lands first, and each regenerates the G family for its own reason — ruling BP6, named at both ends**); `crates/fr-dsn/tests/{structure_scope.rs,scopes.rs,geometry_scopes.rs,lexer.rs,rules.rs,network.rs}`; `tests/reference/<6 G stems>/*`; `tests/reference/cli-large-outline/*`.

**Interfaces consumed:** `fr_board::{Board, CoordinateTransform}` (Plan 2), `fr_settings::RouterSettings` (Plan 4).
**Interfaces produced:** `BoardReadResult::Partial { board, diagnostic }`; `RuleLayerScope::{AllLayers, One}`; a `DsnScanner` with **no** input ceiling.

**The fix list (7 rows).**

| # | mechanism | fix sketch |
|---|---|---|
| **#95** | `Structure.readScope` reads `autoroute_settings` **only when it is the first layer-structure consumer in the scope**. Any earlier `keepout`/`plane` makes the whole scope go unread *and unskipped*, so its closing bracket ends the **structure** scope one level early and the tail is misparsed. **Exporters conventionally write keepouts first.** | Hoist the `AutorouteSettings.readScope` call out of the `if`. **Add a corpus sweep** counting, per fixture, whether the settings were read — the number is the evidence. |
| **#90** | `readIntegerScope`'s failure branch returns `0` and does not consume a second token, so the scope's closing bracket desyncs `AutorouteSettings.readScope`'s depth-unaware loop, which misreads it as ending its own scope. The `0` reaches `RouterSettings` and is written back out. | Fix the token consumption **and** the caller loop **together** — one alone makes a malformed `autoroute` scope parse worse. **Land after #95.** |
| **#94 + #89 + #93** | `createBoard`'s overflow loop divides an **`int`** `scaleFactor` by 10 until it truncates to **0**, for any boundary coordinate ≥ **6 710 886** DSN units; `CoordinateTransform(0,0,0)` is then built without complaint, every written coordinate becomes `Infinity`/`NaN`, every read one `0`, and the outline degenerates to the bare box — **JVM-verified to report `Success`**. #93 (a `(circle …)` bounding box **2× too wide and tall**) halves the threshold. | **One change, one commit:** `scaleFactor` a **`double`** (or clamped ≥ 1); a **rejected** zero/non-finite scale in the `CoordinateTransform` constructor; and `coor[0] / 2` on **all four** circle bounds. **Fixture: check `tests/reference/cli-large-outline` first** — it may already carry a ≥ 6.71 M-unit outline; if it does not, build one (recommendation 9: hand-computed expectation, reviewed here). |
| **#112** | `applyRules` warns "layer not found" and does **not** return, leaving `layerIndex = -1` — the sentinel the branches below read as **"all layers"** — so a stale `.rules` file silently overwrites the **default trace width on the whole board**. Port: `rules_reader.rs::apply_rules` (`Option<usize>`, `None` = all layers). | **Make the sentinel unrepresentable**: a two-variant enum (`RuleLayerScope`), not an `Option`. A named layer that is not found is an **error**, not "all layers". |
| **#91** | `skipScope` returns `false` at EOF, every caller discards it, and `readScope`'s loop then returns `true` — so a DSN truncated inside an unrecognised scope is reported as a **successful** parse of a partial board. | A **third `BoardReadResult` variant** carrying the partial board *and* the diagnostic — **not** a hard failure (the roadmap's correction of the register's binary framing). This is an API change on `BoardReadResult`; every consumer in `fr-core`, `fr-drc` and `freerouting` is updated in the same commit. |
| **#103** | `Network.insertComponent` `return`s — not `continue`s — on a pin naming an absent padstack, so the package contributes its first *n-1* pins and **none** of its keepouts or outlines, and every later item id shifts. | **Reject the component wholesale with a diagnostic** (the honest answer; the register offers `continue` as an equal and it is not). Hard to reach from DSN, **live for the KiCad-JSON reader** (Task 7), which has no such guard. |
| **#86** | The DSN scanner's fixed 16 MiB `char[]` has no refill, so a design over 16 MiB cannot be scanned at all. The port's lexer is already correct; it reproduces the ceiling **on purpose** as `DsnError::InputTooLarge`. | **Delete the deliberate limit.** Accept a 20 MiB input and lex it to the same token stream a 15 MiB one gives. |

**Tests (named, binding).**
- `crates/fr-dsn/tests/structure_scope.rs::an_autoroute_settings_scope_after_a_keepout_is_read` — the inversion of the existing `…_is_never_read`, which is renamed, not deleted.
- `crates/fr-dsn/tests/structure_scope.rs::the_corpus_sweep_counts_boards_whose_settings_were_read` — over `sweep-p3t15.sh`'s 106 fixtures; the before/after counts are the evidence and are pasted into the commit.
- `crates/fr-dsn/tests/scopes.rs::a_malformed_integer_scope_does_not_desync_its_caller`.
- `crates/fr-dsn/tests/structure_scope.rs::a_six_point_seven_million_unit_outline_keeps_its_scale` and `…::a_zero_scale_is_refused_loudly`.
- `crates/fr-dsn/tests/geometry_scopes.rs::a_circle_bounding_box_is_not_twice_too_wide` — hand-computed, four bounds.
- `crates/fr-dsn/tests/rules.rs::a_rules_file_naming_an_absent_layer_leaves_the_default_width_untouched` — a `.rules` naming `B.Cu` against a 4-layer board; **the survey's own directed test**.
- `crates/fr-dsn/tests/scopes.rs::a_truncation_inside_an_unknown_scope_reports_partial` — replaces `read_scope_generic_returns_ok_true_when_truncated_inside_an_unknown_scope`.
- `crates/fr-dsn/tests/network.rs::a_component_with_an_absent_padstack_is_rejected_whole` — and the following components' item ids are unshifted.
- `crates/fr-dsn/tests/lexer.rs::a_twenty_mebibyte_input_lexes_like_a_fifteen_mebibyte_one` — replaces `input_larger_than_the_java_buffer_is_rejected`.

**Evidence / acceptance.** G1 green. ~~**G regenerates** (all 6 `fixtures.txt` rows + the `fr-dsn` round-trip goldens) and **B/R/C move on any board that gains its file's router settings**~~ — **measured false, and ruling BT is what settles it: a family regenerates only when a fix MOVES it, and none of the seven moved anything.** `goldens moved:` **NONE**, with the reasons per row: #95's corpus sweep counts **2 of 106 before and 2 of 106 after** (the corpus's only two `(autoroute_settings …)` files write the scope *before* their first keepout, so neither trips the Java guard); #94's `f64` scale factor agrees with Java's `int` division wherever `resolution / 10^n` is integral, and **exactly one corpus board enters the loop at all** — `Issue676-ch32v-tx118s.dsn`, `resolution 1 000 000`, `maxCoor = 21 590 000`, so `5 * maxCoor = 1.08e8 >= 2^25` and the loop runs **one** iteration: `int` gives `1000000 / 10 == 100000` and `f64` gives `100000.0`, **the same number**, which is why that fixture still MATCHes on every sweep mode. (An earlier draft of this line claimed no corpus board entered the loop, on the strength of `cli-large-outline`'s 2 940 050; that was wrong — `cli-large-outline` does not enter it, but `Issue676` does, and the division being exact is the real reason nothing moved.) #93 is unreachable because **no** `(boundary …)` in the corpus contains a `(circle …)`: over the 111 boundary scopes in the 106 files the direct children are 103 `path` (in 102 files), 6 `rect`, 3 `polygon` and **0 `circle`**, and only the circle count is load-bearing — `DsnCircle::bounding_box` is the only site #93 changes. **`cli-large-outline` was checked first, as this block asks, and NOT re-cut** — it is not a ≥ 6.71 M-unit board, so a synthetic fixture with a hand-computed expectation carries the row instead. `sweep-p3t15.sh` re-run over the 106-fixture corpus, all five modes: **530 pairs, 0 unexpected diffs, exactly the 5 documented XDIFFs** — they did not grow and no fixture changed. G2 (`quality-ab.sh T4`, 29 stems): every quality cell identical to T1's, corpus-median cpu ratio 1.000, all rows within the G2 rules.

**Steps:**
- [x] Commit 1: #95 + the corpus sweep counter.
- [x] Commit 2: #90 (token consumption **and** the caller loop).
- [x] Commit 3: #94+#89+#93 in one commit, with the fixture decision recorded (`cli-large-outline` reused or a new fixture built with its hand-computed expectation).
- [x] Commit 4: #112 — `RuleLayerScope`.
- [x] Commit 5: #91 — `BoardReadResult::Partial` and every consumer.
- [x] Commit 6: #103.
- [x] Commit 7: #86.
- [x] Commit 8: **nothing to regenerate** (ruling BT); `sweep-p3t15.sh` clean; `goldens moved: NONE`.
- [x] Register: seven rows → `fixed: T4`.

**Commit message:** `fix(dsn): read integrity — #95/#90 the settings scope, #94+#89+#93 the coordinate scale, #112 the layer sentinel, #91 partial reads, #103, #86`

---

### Task 5: termination — normalisation and net numbers — survey group 5

**Ordering inside the task is fixed** (survey §10.1 constraint 3): **#71 first**, then re-measure whether **#76** needs anything, then **#106**.

**Files:** `crates/fr-board/src/board/trace_normalize.rs`; `crates/fr-board/src/searchtree/{mod.rs,shape_search_tree.rs}` (`overlapping_tree_entries`); `crates/fr-board/src/items/mod.rs` (`get_connection_items`); `crates/fr-board/src/board/mod.rs` (`reduce_nets_of_route_items`, `assign_net_no`, `remove_from_net`); `crates/fr-dsn/src/parser/wiring.rs`; `crates/fr-board/tests/{trace_normalize.rs,board.rs}`; `crates/fr-dsn/tests/wiring.rs`; fixtures `p8t13-via-net-numbers.dsn` + its control (already committed, Plan 8 Task 13).

**Interfaces consumed:** `fr_board::searchtree::ShapeSearchTree` (Plan 2), the Plan 8 Task 13 fixtures.
**Interfaces produced:** `overlapping_tree_entries` returning a **fresh** collection (the aliasing becomes unrepresentable).

**The fix list (3 rows).**

| # | mechanism | fix sketch |
|---|---|---|
| **#71 + #76 + #106** | `overlappingTreeEntries` **appends** to the caller's collection and never clears it; `PolylineTrace.split` re-reads into the same list and restarts the iterator; and `Item.getConnectionItems` walks the contacts **with no visited set** — so a two-rail four-rung ladder on one net makes `normalizeAllTraces` **never return**, and every DSN read ends with that call. Port: `trace_normalize.rs`, with an `#[ignore]`d unbounded reproduction. | Return a **fresh** collection from `overlapping_tree_entries` — that makes the aliasing unrepresentable, which is the roadmap's correction of the register's "either/or" — **then** add the visited set to the contact walk. **Do #71 first and re-measure whether #76 needs anything more.** The ignored test becomes a **terminating assertion with a literal answer**. |
| **#211 + #45** | The trace arm of `reduceNetsOfRouteItems` puts its `break` **outside** the net loop where the via arm's is inside, so one visit can strip **every** net from a route item; `assignNetNo` on a multi-net item overwrites only `netNumbers[0]`; and `removeFromNet`'s loop has no `break`, so a duplicate is removed at its **last** occurrence. | Move the break **inside** the net loop; replace the whole array or refuse the call; add the missing `break`. **Land together** — #211 is what creates multi-net items. Note the port reproduces Java's placement *deliberately* (a Plan 7 Task 8b parity repair), so this **moves it back**, and the Plan 7 marker is re-pointed rather than deleted. |
| **#105** *(ruling BI)* | `Wiring.readViaScope`'s net-number loop omits its `++currentIndex`, so a multi-subnet via's net array is padded with `0` — and `calculateAllIncompletes`' `nets.get(0)` is then `Vector.get(-1)`, so **every autoroute pass throws and `AutorouteBatchLoop.run` retries for ever. The jar hangs**; one changed token separates it from a 1 995-byte routed SES. Java: `Wiring.java:684-687`; `DesignRulesChecker.java:558`. | **Add the `++currentIndex`.** Fixture and control already exist (Plan 8 Task 13), so this is the **cheapest T1 row in the catalogue**. Upstream-PR candidate (recommendation 8's list). |

**Tests (named, binding).**
- `crates/fr-board/tests/trace_normalize.rs::a_two_rail_four_rung_ladder_normalizes_and_terminates` — the `#[ignore]` is removed and the test asserts a **literal** final trace count, hand-computed and reviewed.
- `crates/fr-board/tests/trace_normalize.rs::overlapping_tree_entries_returns_a_fresh_collection` — the caller's collection is untouched across two calls.
- `crates/fr-board/tests/board.rs::a_cyclic_contact_graph_terminates_with_a_visited_set` — #106's half, measured **after** #71 lands so the re-measure is recorded.
- `crates/fr-board/tests/board.rs::one_visit_reduces_only_the_visited_net` — replaces `one_visit_reduces_two_nets_…`.
- `crates/fr-board/tests/board.rs::assign_net_no_replaces_the_whole_array` and `…::remove_from_net_removes_the_first_duplicate` — the three existing `assign_net_no_*` tests are re-pointed.
- `crates/fr-dsn/tests/wiring.rs::a_multi_subnet_via_carries_every_net_number` — against `p8t13-via-net-numbers.dsn`, and the control asserts the single-subnet case is unchanged.
- `crates/freerouting/tests/cli_e2e.rs::the_via_net_number_fixture_routes_instead_of_hanging` — the 1 995-byte SES, asserted by size and by re-read.

**Evidence / acceptance.** G1 green. **B/R likely move** — the re-walk order feeds every shove — and `p2t11` mode 8 is re-golden. **`p8t1` loses its one XDIFF row** for the via-net fixture; the row is deleted and the deletion is named. #211+#45 is **latent on the corpus** (no committed item is on two nets), so its evidence is the directed tests plus the argument, and its Δrefs is `U`. G2: no stem may regress; the ladder fixture's termination is the headline. **`p6t3` mode 5's `timeout(1)` bound is NOT removed here** — that is #162, Task 8.

**Steps:**
- [ ] Commit 1: #71 — fresh collection; two directed tests; **re-measure #76 and paste the measurement**.
- [ ] Commit 2: #76/#106 — whatever the re-measure says is still owed, with the visited set; the ignored test un-ignored.
- [ ] Commit 3: #211 + #45 together; the Plan 7 Task 8b marker re-pointed.
- [ ] Commit 4: #105 — one token; two directed tests; the `p8t1` XDIFF row deleted.
- [ ] Commit 5: regenerate B/R where they moved; `p2t11` re-golden; `goldens moved:`.
- [ ] Register: #71/#76/#106, #211/#45, #105 → `fixed: T5`; #105 flagged **upstream-PR**.

**Commit message:** `fix(board): termination and net numbers — #71+#76+#106 normalizeAllTraces terminates, #211+#45 the net arms, #105 the via net-number index`

---

### Task 6: the reachable crash guards — survey group 6

**Guards at the defect, never a `catch_unwind` wrapper** (Plan 7 ruling 7 / scan ruling 9, still binding: do not add recovery Java lacks — *a guard is not recovery*).

**Files:** `crates/fr-router/src/board_ext/routing_board_ext.rs` (`insert_forced_trace_polyline`); `crates/fr-router/src/autoroute/path/locator_any_angle.rs` (`calculate_next_trace_corners`); `crates/fr-router/src/autoroute/engine.rs` (`remove_incomplete_expansion_room`, `ExpansionDrill::calculate_expansion_rooms`); `crates/fr-router/src/autoroute/drill_page.rs` (`get_drills`); `crates/fr-router/src/autoroute/control.rs` (`init_net`); **the ten-site guard cluster, enumerated (ruling BP11 — no bare crate directories, so explicit-path staging and pairwise conflict detection both work):** `crates/fr-board/src/board/communication.rs` (#67 `host_is_old_kicad`), `crates/fr-settings/src/router_settings.rs` (#123 `get_horizontal_trace_costs` / `get_vertical_trace_costs`), `crates/fr-board/src/structure/component.rs` (#47 `Component::change_side`, #49 `Components::get`/`get_mut`), `crates/fr-board/src/library/package.rs` and `crates/fr-board/src/library/logical_part.rs` (#42 `Packages::get` / `LogicalParts::get`), `crates/fr-board/src/library/board_library.rs` (#43 `remove_via_padstack`), `crates/fr-board/src/items/drill.rs` (#52 `Pin::get_trace_exit_restrictions`), `crates/fr-geometry/src/polyline.rs` (#22 `remove_overlaps`/`from_lines`, #25 `corner_count`), `crates/fr-geometry/src/tile_shape.rs` (#24 `rotate_approx`) — **sixteen source paths in this task's Files line in total**; `crates/fr-router/tests/{drill.rs,control.rs}`; `crates/fr-board/tests/*`; `crates/fr-geometry/tests/polyline.rs`; two **new** synthetic fixtures under `crates/fr-router/tests/data/`.

**Interfaces consumed:** `fr_router::autoroute::{AutorouteEngine, ExpansionDrill, DrillPage, AutorouteControl}` (Plan 6).
**Interfaces produced:** none — every fix is a guard behind an unchanged signature, plus two new fixtures.

**The fix list (6 rows).**

| # | mechanism | fix sketch |
|---|---|---|
| **#185** | `insertForcedTracePolyline` dereferences a possibly-`null` `newTrace` at `:756` and guards **the same variable** at `:791`; no `catch` covers `:756`, so the nearest handler is a bare `FAILED` — **the whole connection is abandoned where the guard would have skipped one segment.** | Move `:791`'s null test **up to `:756`**. Latent on all 1 621 rows of `p6t15b-insert-forced.txt`, so it needs a **new synthetic fixture**: a resample that brings a polyline's two ends together. |
| **#181** | `calculateNextTraceCorners` builds a `FloatLine` from a possibly-null corner and dereferences it one line later; the author guarded the same value 40 lines down. `autorouteConnection` catches it and degrades the whole connection to `FAILED`. Java: `FoundConnectionLocatorAnyAngle:287`. | Hoist the `resultCorner != null` test to `:287` and **skip the correction loop**. Needs a **new synthetic fixture**: two parallel lines whose intersection is null. |
| **#169** | `removeIncompleteExpansionRoom` dereferences a lazily-created list where its three siblings guard; `ExpansionDrill.calculateExpansionRooms` uses the bare constructor, so the NPE is swallowed by #166's `catch` and read as "blocked". **On an engine that has never had an incomplete room added, no drill can ever be built** — JVM-verified: **0 drills vs 13, 28 swallowed NPEs**. | Guard the field **and** have `ExpansionDrill` call `addIncompleteExpansionRoom`. **Both.** |
| **#168** | A cancelled `splitToConvex` makes `DrillPage.getDrills` throw **and** leaves the page memoised as having **no drills**, so a page interrupted once answers "no drills here" for the rest of the connection — **silently removing every via candidate on it.** | Install the list **only after** the split succeeds. |
| **#173** | `AutorouteControl.initNet`'s null-net arm completes only for `netNumber <= 0`; a **positive unknown net** throws two lines later, and `RoutingBoard.java:1023` builds a control from a pin's net number — so a stale net number kills the connection. | Give the null-net arm its **own half-width fallback**. Better than moving the test: the current arm reads **net 1's** widths, which is itself arbitrary. |
| **#67, #123, #47, #49, #42, #43, #52, #22, #24, #25** | Ten unguarded dereferences and indices on paths a real design reaches: `Integer.parseInt` on `host_version`'s first digit run (KiCad-facing, thrown out of a predicate every board load calls); two cost accessors with no null guard where their two siblings answer `1.0`; `changeSide` on an unplaced component (NPE **after** flipping `onFront`, leaving it half-mutated); `Components.get(0)` on the "no component" sentinel; `Packages.get`/`LogicalParts.get` with no bounds check where `Padstacks.get` has one; `removeViaPadstack` before any via padstack exists; `Pin.getTraceExitRestrictions` dereferencing the component before its own null guard; `Polyline.removeOverlaps` reading index **−1** (~**11 %** of random small-pool line arrays; six lines `h,v,h,v,h,v` is minimal) which **aborts the routing pass**; `TileShape.rotateApprox`'s two-corner branch at an invalid index; `Polyline.cornerCount()` returning **−1**, which flows into `new IntPoint[-1]`. | Add **the guard the sibling already has, at the defect**. #47 also flips `onFront` **after** the guard. **#22 changes what a degenerate polyline normalises to, so it is the one with blast radius** and gets its own commit and its own A/B. |

**Tests (named, binding).** Every one of the sixteen `#[should_panic]`/`expect` reproductions in `fr-router`, `fr-board`, `fr-settings` and `fr-geometry` is **inverted in place** — the `#[should_panic]` attribute is removed and the test asserts the guarded answer:
- `crates/fr-router/tests/insert_forced.rs::a_degenerate_resample_skips_one_segment_not_the_connection` (new fixture `t6-resampled-polyline.dsn`, hand-computed expectation reviewed here).
- `crates/fr-router/tests/locator_any_angle.rs::a_null_intersection_skips_the_correction_loop` (new fixture `t6-parallel-lines.dsn`).
- `crates/fr-router/tests/drill.rs::a_virgin_engine_yields_thirteen_drills` — replaces `a_virgin_engine_yields_no_drills_at_all`; the literal **13** is the JVM-verified count.
- `crates/fr-router/tests/drill.rs::a_stopped_split_does_not_memoise_an_empty_page` — replaces `split_to_convex_stops_when_the_stop_check_trips`.
- `crates/fr-router/tests/control.rs::a_positive_unknown_net_gets_the_half_width_fallback` — replaces `a_positive_net_the_board_does_not_have_throws_like_java`.
- Ten named inversions, one per site, each keeping its existing test name with `_throws_like_java` → `_is_guarded`.
- `crates/fr-geometry/tests/polyline.rs::remove_overlaps_on_a_degenerate_array_normalises_to_a_literal` — #22's blast radius, with the hand-computed answer.

**Evidence / acceptance.** G1 green. Fourteen of the sixteen are **U-only**. **#22 is R/B likely** and gets its own G2 row: the 11 %-of-random-arrays measurement is re-run and pasted, and any batch stem whose SES moves is named. **#169's `0 → 13` drills is the headline number** and is asserted as a literal. G2: incompletes must fall on at least one stem (#185 and #181 each recover a connection that was being abandoned wholesale).

**Steps:**
- [ ] Commit 1: the two new synthetic fixtures with their reviewed expectations.
- [ ] Commit 2: #185. Commit 3: #181. Commit 4: #169. Commit 5: #168. Commit 6: #173.
- [ ] Commit 7: nine of the ten guard-cluster sites (all but #22).
- [ ] Commit 8: **#22 alone**, with its own A/B and the named stems.
- [ ] Commit 9: regenerate R/B if #22 moved them; `goldens moved:`.
- [ ] Register: six rows → `fixed: T6`; #27's neighbour note left for Task 12.

**Commit message:** `fix(router): the reachable crash guards — #185, #181, #169 (0 -> 13 drills), #168, #173 and the ten-site cluster`

---

### Task 7: KiCad JSON — validation, identity, numbering — survey group 7

**Ordering inside the task is fixed** (survey §10.1): **#284 with #286** (#284 currently *masks* #286 — a pad whose layers match nothing silently borrows a valid padstack instead of crashing; fix the identity and the crash surfaces), and **#282 → #285** (once pin names cannot be null, the package-dedup fallback is unreachable). **#280 must land before Task 24** (~30 occurrences of `HashMap` emulation die with it).

**Files:** `crates/fr-dsn/src/kicad/{reader.rs,dto.rs,writer.rs}`; `crates/fr-dsn/src/kicad/java_hash.rs` (`JavaStringSet`, `JavaStringMap`, `java_hash_iteration_order`, `java_string_hash` — **made DEAD here, DELETED at Task 24. Ruling BP16 settles the plan's earlier "deleted here or made dead here": this task removes the last call site and leaves the modules in place with a `// dead: killed by #280, deleted at T24` header, and Task 24's ~150-occurrence count includes them**); `crates/fr-dsn/src/kicad/npe.rs` (`JavaNpe` — **same: dead here, deleted at Task 24**); `crates/fr-dsn/tests/kicad_*.rs`; `crates/fr-core/tests/data/p8t8-kicad-read-{a,b}.txt` and `p8t10-kicad-writer.txt` (probe transcripts re-cut or retired); `tests/reference/cli-kicad-ecc83-json/*`, `tests/reference/cli-kicad-complex-hierarchy-json/*`.

**Interfaces consumed:** `fr_dsn::{BoardReadResult, ReadDiagnostic}` (Task 4's third variant — this task's diagnostics use it), `fr_board::Padstacks` (Plan 2).
**Interfaces produced:** a DTO layer that **rejects** a null name or a null array element; padstacks keyed on shapes + layer set + drill; insertion-ordered net numbering.

**The fix list (6 rows).**

| # | mechanism | fix sketch |
|---|---|---|
| **#282 + #283 + #287** | A `null` name or a `null` **array element** is stored verbatim and crashes hundreds of lines later, inside a JDK collection: a null layer name dies at `:545` only if the board also has pads with a non-empty `layers` list; a null net name dies inside `Nets.get`'s walk; a null component `reference` dies inside `ConcurrentSkipListMap.put` via `Component.compareTo`. **One tolerant site (`equalsIgnoreCase(null)` is `false`) against three throwing ones, in one reader.** Java: `KiCadJsonReader.java:114, :131, :543-549, :625-634, :678, :288, :940`. | **Validate at the DTO boundary**: reject a null name and a null array element with a diagnostic **naming the file, the section and the offending object**. That deletes the null side-tables (`net_name_is_null`, `package_pin_names`), the NPE emulation (`JavaNpe`) and half the probe's rows. |
| **#286** | `DrillItem.tileShapeCount` returns a **negative** number for a padstack with no shape on any layer (`toLayer - fromLayer + 1` = `-layerCount`), and the insert then allocates `new TileShape[-n]`. Reachable from a pad whose `layers` match nothing and from a via with `startLayerIndex > endLayerIndex`. The message is the bare number: `Exception occurred: -2`. On the `importSession` path Java keeps a via that is **in the item list and in no search tree**. | **Reject an all-`null` shape array in `Padstacks.add`, with a message naming the pad.** Then the half-inserted-via divergence disappears rather than being pinned. |
| **#284** | The generated padstack key encodes **neither the layer span nor the drill**, and the lookup is **case-insensitive** — so the second pad to ask for a name **silently inherits the first pad's shapes**. Measured: a middle-layer 1×1 pad gets a three-layer padstack; a drilled and an undrilled pad of the same size share one padstack **and the first one's `attachAllowed`**; `Round` names drop `size.y` entirely. **It also masks #286.** | Key the padstack on the **shape array + layer set + drill**, and keep the generated **name as a display artefact**. The name reaches the SES, so this **re-baselines every KiCad-sourced output**. |
| **#285** | The per-component `catch (Exception)` is a **package-deduplication fallback**, not a component skip: a `null` pad name throws inside `arePackagePinsIdentical` and the ladder then adds a package under the raw `footprint` string — so a board whose pads have no `name` gets **one duplicate package per component** (three components → three `NONAME` packages), silently. | **Falls out of #282's DTO validation**: with pin names non-null the fallback is unreachable and the side table goes. **Null-check the pin name in the comparison as well** — belt and braces, because the comparison is public. |
| **#280** | The **net numbers** of every auto-registered net are `java.util.HashSet` iteration order, i.e. a function of `String.hashCode` — and that numbering is what every `netNumbers[]`, every DSN `(net …)` scope and every SES wire carries. `Issue649`'s thirteen pad nets come back in nothing like declaration order. Deterministic, but **arbitrary**; the port reproduces it by **rebuilding `HashMap`'s bucket layout**, with a `debug_assert!` guarding treeification (peak bucket 4 against the 9 a treeify needs). | **Use insertion order** (`LinkedHashSet`). One word in Java; in the port it **deletes the whole `HashMap` emulation** and the treeification hazard the Plan 8 hand-off parks at §6 task 9. **Re-baselines every KiCad-JSON board's net numbers and therefore its SES.** |
| **#281's `OutlineJson.clearance`** | The DTO field is parsed and `KiCadJsonReader:310` **hard-codes the value to `1`** — an intended feature that never reached the board. (The other three §9.1 DTO fields — `NetClassJson.netNames`, `NetJson.id`, `LayerJson.index` — are a **wire contract** and must keep round-tripping; only the clearance is a fix.) | **Wire it or drop it** — decide here and record which. Default: **wire it**, because the field is a real design rule and the writer already round-trips it; dropping it would make the DTO a reader's private struct, which survey §9.1 forbids. |

**Tests (named, binding).**
- `crates/fr-dsn/tests/kicad_dto.rs::a_null_layer_name_is_rejected_with_the_section_named`, `…::a_null_net_name_is_rejected…`, `…::a_null_component_reference_is_rejected…`, `…::a_null_array_element_is_rejected…` — four, each asserting the diagnostic **text** contains the file, the section and the object.
- `crates/fr-dsn/tests/kicad_padstacks.rs::two_pads_of_the_same_name_and_different_layers_get_different_padstacks`, `…::a_drilled_and_an_undrilled_pad_do_not_share_attach_allowed`, `…::a_round_pad_name_carries_size_y`.
- `crates/fr-dsn/tests/kicad_padstacks.rs::an_all_null_shape_array_is_refused_naming_the_pad` — #286, and the `java_drill_item_tile_shape_count` emulation is deleted with it.
- `crates/fr-dsn/tests/kicad_packages.rs::three_components_with_unnamed_pads_yield_one_package` — replaces the three-`NONAME` reproduction.
- `crates/fr-dsn/tests/kicad_nets.rs::net_numbers_follow_declaration_order` — over `Issue649`'s thirteen pad nets, asserting **1..13 in declaration order**; the `java_hash_iteration_order` reproduction test is deleted by name.
- `crates/fr-dsn/tests/kicad_round_trip.rs::the_outline_clearance_round_trips` — #281.

**Evidence / acceptance.** G1 green. **C moves on the 2 KiCad stems** — twice over: #284 re-names every padstack the SES carries, and #280 renumbers every net. `goldens moved:` names both, with the net-number mapping (old → new) for `Issue649` pasted in full. `p8t7` re-golden; the `P8T8Probe`/`P8T10Probe` transcripts are **re-cut as port goldens or retired** — #282's DTO validation turns several of their rows into rejections, and the surviving rows are the ones that still describe the port. **The `debug_assert!` guarding treeification is deleted** and its deletion named. `JavaStringSet`, `JavaStringMap`, `java_hash_iteration_order`, `java_string_hash`, `JavaNpe`, `net_name_is_null`, `package_pin_names` and `java_drill_item_tile_shape_count` become **dead** here — their last call sites go and each module gains a `// dead: killed by #280/#282, deleted at T24` header — and **Task 24 deletes them** (ruling BP16; do not delete a shim before its fix has merged — survey §8). This task therefore deletes **no** shim file, and a `git status` showing one deleted here is a stop-and-report. G2: the two KiCad stems' incompletes must not rise.

**Steps:**
- [ ] Commit 1: #282+#283+#287 — DTO validation and the four diagnostics.
- [ ] Commit 2: #285 — the fallback goes with the side table; the explicit null check kept.
- [ ] Commit 3: #284 **and** #286 together (the mask and the crash).
- [ ] Commit 4: #280 — insertion order; the emulation made dead; the `debug_assert!` deleted.
- [ ] Commit 5: #281's outline clearance, wired, with the decision recorded.
- [ ] Commit 6: re-cut the 2 KiCad C-stems and the two probe transcripts; `goldens moved:` with the net-number mapping.
- [ ] Register: six rows → `fixed: T7`; survey §9.1's #281 DTO-contract row keeps `keep` for its other three fields.

**Commit message:** `fix(kicad): a malformed board is told so — #282+#283+#287 DTO validation, #284+#286 padstack identity, #285, #280 insertion-ordered nets, #281 outline clearance`

---

### Task 8: rooms, doors and expandable identity — survey group 8

**The largest T2 group, and the one whose ordering matters most.** Survey §10.1 constraint 6: **#160 + #161 → #171 (+ #170)** — both are `JavaTreeSet` drops, and fixing the neighbour comparator changes which elements ever reach the maze queue, so measuring #171 first measures noise. Survey §10.2: **#160/#161 and #162 change the door set of the same rooms; land them in separate commits with separate A/Bs, #162 second**, because #162 is the riskiest T1 row (0.4 % of completions) and #160/#161 the largest T2 one (481 drops per 2 000).

**The Task 17 hand-off, made mechanical (ruling BP7).** Task 17 is dispatched immediately before this one *because its answer may subsume rows here*, and until now that path had no mechanism. It has one: **Task 8 opens by reading `docs/plan-9-prep/stale-index-report.md` and recording, in a table in its first commit message, one line per row of the fix list below — `unaffected` / `subsumed by #193's finding <n>` / `narrowed: <what changed>`.** A row marked **subsumed** is **dropped from this task's fix list** (its register status becomes `pinned` with the report's finding cited, not `fixed: T8`), and the drop is reported to the controller with the report's paragraph quoted. A row marked **narrowed** keeps its slot with its mechanism restated. **Task 17 still does not fix #193**, and no row here is *added* by it. If every row reads `unaffected`, that sentence is the record and the fix list stands at nine.

**Files:** `crates/fr-router/src/autoroute/{tree_ext.rs,sorted_neighbours.rs,engine.rs,maze_list_element.rs,maze_search.rs,expansion_room.rs,incomplete_room.rs,drill_page.rs}`; `crates/fr-board/src/searchtree/shape_search_tree_90.rs`; `crates/fr-router/tests/{tree_ext.rs,sorted_neighbours.rs,sorted_neighbours_regimes.rs,engine_rooms.rs,maze_list_element.rs,maze_search.rs,expansion_rooms.rs,drill.rs,incomplete_room.rs}`; a **new 90-degree fixture** under `crates/fr-router/tests/data/`; `scripts/differential/run.sh` (`p6t2` retires here).

**Interfaces consumed:** `fr_board::searchtree::{ShapeSearchTree, ShapeSearchTree90Degree, ShapeSearchTree45Degree}` (Plan 2), `fr_router::autoroute::AutorouteEngine` (Plan 6).
**Interfaces produced:** a **total** `SortedRoomNeighbour` ordering and a **total** `MazeListElement` ordering (both of which make `JavaTreeSet` replaceable by `BTreeSet` — Task 24 collects it); a per-engine **stable, injective id** for every expandable object.

**The fix list (9 rows).**

| # | mechanism | fix sketch |
|---|---|---|
| **#159** | `ShapeSearchTree90Degree.completeShape` **drops** a room it decided to ignore where the base class and the 45-degree override **keep** it — and `completeExpansionRoom` passes exactly that pair on **every** room completion. On a 90-degree board a room whose only overlap is the door it came through **expands to nothing**. Measured `[4, 4, 0]` across the three regimes. | Add the base class's fallthrough **verbatim** — the 45-degree sibling **is** the specification. Needs a **90-degree fixture**; nothing in the corpus exercises the regime. **This is also where I1 (#295) is answered**: with #159 fixed, measure whether a periodic exhaustive tree still buys anything, and write the answer into the register row. |
| **#160 + #161** | `SortedRoomNeighbour.compareTo` is **not a total order** and the `TreeSet` it feeds silently drops elements it calls equal — **a door the room really has is never built.** Measured **481 drops in 2 000 cases**. #161 is the same comparator's final tie-break subtracting a **room** id from an **item** id. | Make it a total order: **compare last corners whenever the first-corner distances tie, apply or drop `compareFrom` consistently, and compare the object *kind* before the id.** Then `JavaTreeSet` → `BTreeSet` and no neighbour is lost. **One fix, both rows.** |
| **#164** | `removeCompleteExpansionRoom`'s `otherRoom(room)` binds the **narrowing** overload at compile time, so the `null` check below skips **every door whose far side is an incomplete room — which is most of them**. The skip is what keeps the method alive: the code past it indexes `touchingSides[1]` with **no length check**. | Take an `ExpansionRoom` parameter **and** length-check `touchingSides`. **Both** — fixing only the overload turns a silent skip into an `ArrayIndexOutOfBoundsException`. |
| **#163** | `Sorted45DegreeRoomNeighbours…OfObstacleExpansionRoom` never advances `currentCorner`, so the degenerate-side guard compares every side's end corner against the corner the walk **started** at, and the **last side is always skipped** — eight-sided obstacle rooms get seven doors. JVM-verified. | `currentCorner = nextCorner;` at the foot of the loop. **7 → 8.** |
| **#171 + #170** | A four-key tie in `MazeListElement.compareTo` answers `0` and `TreeSet.add` **discards the newcomer whole** — a different backtrack path at the same cost is lost, not merged — and `door.getId()` is a **hash**, so two *different* doors collide into the tie. #170: a **`NaN`** `sortingValue` falls through to the next key and the relation stops being transitive. | Add the remaining fields to the comparison (or keep the cheaper backtrack **explicitly**); **reject** a non-finite `sortingValue` at the three `add` sites rather than ordering it — the roadmap's correction: *a NaN cost is an upstream bug, not a thing to sort*. **Land after #160/#161.** |
| **#165 + #166** | `completeExpansionRooms` is **not** the set of complete rooms that exist: rooms are constructed **before** they are known to survive, and two paths abandon them **still wired to live doors** — never validated, never invalidated, never removed from the tree — and `completeExpansionRoom`'s scan can hand `completeShape` a room that is **not in the tree it is querying**. #166's `catch` returns an **empty** collection for rooms already committed to the database. | Take the room id **after** the commit; give `addCompleteRoom`'s `null` path a `removeAllDoors`; **hoist `result` out of the `try` and return it from the `catch`.** |
| **#178** | `MazeSearchEngine.init` ignores the `boolean` its own overridden `add` returns, so `startOk` can be `true` with an **empty** queue — the caller pays for a whole engine construction, a `reduceTraceShapesAtTiePins` pass and a full round of room completion to learn the queue was empty. | `if (mazeExpansionList.add(newListElement)) { startOk = true; }` — makes `getInstance` answer `null` for a fanout window that is too tight, which **is** what "initialisation failed" means. |
| **#156 + #167 + #158** | Three room/page identities that are **hashes over mutable state**: `ObstacleExpansionRoom.getId` packs the shape index into the item id with an **or** (index ≥ 1024 aliases, item id ≥ 2²¹ overflows) and that id is the **third sort key** of `MazeListElement.compareTo`; `DrillPage.getId` hashes a field `getDrills` **overwrites**, so a queued page silently changes its own sort key; `IncompleteFreeSpaceExpansionRoom.getId` NPEs on the whole-plane room and moves when its shape is replaced. | **One fix:** every expandable object gets a **stable, injective id — a per-engine counter**, as `CompleteFreeSpaceExpansionRoom` already has. Ids feed the maze tie-breaks, so **routing moves**; the current ids are *provably wrong*, not merely arbitrary. |
| **#162** | `calculateNewIncompleteRooms` does not terminate and dies with `OutOfMemoryError` when a room's shape has more border lines than its `toSimplex()` does. **Reachable from production at 0.4 % of room completions.** The port does not guard it either, so the generators carry a `timeout(1)`. | Compute `roomSimplex` **once, in the constructor**, and derive every `touchingSideNo` from it. **Not the loop bound** — that terminates on the wrong side. `p6t3` mode 5 currently skips these calls; **the fix makes mode 5 full-coverage and that is the acceptance**, and the generators' `timeout(1)` bound is removed with its removal named. |

**Controller note from Task 6 (ruling BX(b)) — #162 is now reachable through drill pages, and one test is pinned small because of it.** Task 6's #169 fix (`ExpansionDrill` builds its seed room through `addIncompleteExpansionRoom`, and `removeIncompleteExpansionRoom` takes the field guard its siblings have) makes drills compute for real on a virgin engine, and that removed an accidental shield in front of **#162**. Measured by the T6 implementer and **confirmed by the T6 reviewer with an instrumented turn counter**: on `P6T9Probe`'s bare board a drill page that is a *sub*-rectangle of the board runs away inside `complete_expansion_room` → `calculate_doors` → `SortedRoomNeighbours::calculate_new_incomplete_rooms` → `TileShape::intersection`, and the loop's own state at the moment it runs away is **`roomSimplex` lines = 3 against `from_room` border lines = 8** — #162's trigger exactly, not a new defect. **No new register row is opened; #162 already owns it and this task already owns #162.** The pre-existing-ness is proven independently (the same non-termination reproduces on the unfixed tree with the incomplete-room list seeded, which is the identical engine state). Two consequences for this task: (a) #162's acceptance gains a second producer — a drill page on a 2×2 page grid, not only `p6t3` mode 5; (b) `crates/fr-router/tests/board_ext.rs::additional_update_after_change_invalidates_the_drill_pages_of_every_tree_shape` was moved from `P6T9Probe`'s ±10 000 box to **±5 000** (the largest box whose page grid is 1×1 and therefore still terminates) and now asserts **7 real drills** where it used to assert `drills().is_some()` on an empty memo. **Restoring that test to ±10 000 is part of #162's fix here**, and if #162 is dropped or narrowed by the Task 17 subsumption table, say so against that test explicitly.

**Tests (named, binding).**
- `crates/fr-router/tests/tree_ext.rs::the_ninety_degree_override_keeps_the_room_it_ignores_by_shape` — replaces `only_the_90_degree_override_drops_…`; asserts `[4, 4, 4]` across the three regimes.
- `crates/fr-router/tests/sorted_neighbours.rs::the_neighbour_comparator_is_a_total_order` — the 2 000-case generator with **0 drops** (was 481), and `…::a_room_id_is_never_subtracted_from_an_item_id`.
- `crates/fr-router/tests/engine_rooms.rs::a_door_onto_an_incomplete_room_is_not_skipped` and `…::touching_sides_is_length_checked`.
- `crates/fr-router/tests/sorted_neighbours_regimes.rs::an_eight_sided_obstacle_room_gets_eight_doors` — the literal 7 → 8.
- `crates/fr-router/tests/maze_list_element.rs::two_paths_at_the_same_cost_are_both_kept` and `…::a_non_finite_sorting_value_is_refused_at_add`.
- `crates/fr-router/tests/engine_rooms.rs::an_abandoned_room_leaves_no_live_doors` and `…::a_committed_room_survives_the_catch`.
- `crates/fr-router/tests/maze_search.rs::an_empty_queue_makes_get_instance_answer_none`.
- `crates/fr-router/tests/expansion_rooms.rs::every_expandable_id_is_stable_and_injective` — one property test across the three id sources, including index ≥ 1024 and item id ≥ 2²¹.
- `crates/fr-router/tests/incomplete_room.rs::the_whole_plane_room_has_an_id`.
- `crates/fr-router/tests/sorted_neighbours.rs::calculate_new_incomplete_rooms_terminates_on_the_pinned_trigger` — the `#[ignore]`d pin becomes a terminating assertion.

**Evidence / acceptance.** G1 green. **R and B move on every board** — this is the second-largest churn after Task 2 — and `goldens moved:` carries one line per fix with its direction. **`p6t2` (2 000 random seed rooms per regime) RETIRES here** (BL7): run it once more, record the final MATCH count, delete the pair, keep the Rust half's 2 000-case assertion as `sorted_neighbours.rs`'s total-order test. `p6t3` modes 1-3, 5 and 8 re-golden; **mode 5 becomes full-coverage** and the generators' `timeout(1)` is removed. **I1 (#295) is answered here** and its register row is closed with the measurement. G2: this is the group whose incomplete-connection column is expected to move most — **incompletes must fall on at least one stem and rise on none**, and the 481-drops-to-0 and 7-to-8-doors numbers are pasted.

**Steps:**
- [ ] Commit 1: the 90-degree fixture with its reviewed expectation, **and** the Task 17 subsumption table (one line per fix row: unaffected / subsumed / narrowed) in the commit message.
- [ ] Commit 2: #159, and the I1 measurement written into #295.
- [ ] Commit 3: #160 + #161, with its own A/B.
- [ ] Commit 4: #162, with its own A/B; `p6t3` mode 5 full-coverage; the `timeout(1)` removed.
- [ ] Commit 5: #164. Commit 6: #163. Commit 7: #171 + #170. Commit 8: #165 + #166. Commit 9: #178. Commit 10: #156 + #167 + #158.
- [ ] Commit 11: retire `p6t2` (final MATCH count recorded); regenerate R/B; `goldens moved:`.
- [ ] Register: nine rows → `fixed: T8`; #295 closed with its answer; #162 struck from `docs/plan-8-handoff.md` §7's known-limitations list (Task 25 carries the edit).

**Commit message:** `fix(router): rooms, doors and expandable identity — #159, #160+#161 (481 drops -> 0), #162, #164, #163, #171+#170, #165+#166, #178, #156+#167+#158`

---

### Task 9: the optimizer stage and the pass loop — survey group 9

**Ordering inside the task is fixed** (survey §10.1 constraint 9): **#214 → #227 → #202.** #214 is the "one flag means five things" defect; #227's stage-scoped stop is the same repair one level up; #202 only becomes observable once #227 makes the stage do work. **#230+#267 land with #254 (Task 20)** — the survey pairs them; here we fix the *fields*, Task 20 fills `phases.optimizer.passes_completed`.

**Files:** `crates/fr-router/src/pipeline/{batch_loop.rs,optimizer.rs,run.rs,batch_autorouter.rs,pass_runner.rs,stop.rs}`; `crates/fr-core/src/manifest.rs`; `crates/freerouting/src/commands/route.rs`; `crates/fr-router/tests/{optimizer.rs,batch_loop.rs,pass_runner.rs,stop_and_progress.rs}`.

**Interfaces consumed:** `fr_router::pipeline::{RouterStop, StopRequestState, BatchLoopResult, OptimizerResult, PipelineResult}` (Plans 7–8).
**Interfaces produced:** `BatchLoopExit`; `BatchLoopResult::exit()`; a **stage-scoped** optimizer stop; `autoroute_items` returning `Vec<(ItemId, NetIndex)>`; separate `passes_completed` fields for the two stages.

**The fix list (7 rows).**

| # | mechanism | fix sketch |
|---|---|---|
| **#214** | A **normal** end of routing reports `CANCELLED`, not `FINISHED`: `AutorouteBatchLoop.java:571` fires `FINISHED` only when the stop flag is still clear, and every ordinary exit raises it first. A CLI run that does exactly what it was asked ends `CANCELLED`, and **no consumer can tell it from a user cancellation.** | Carry the **exit reason** out of the loop (`BatchLoopResult` is already shaped for it) and report `FINISHED` for `maxPasses`/completion. **Prerequisite for #227's stage-scoped flag** — both are the same defect. |
| **#227** | **The optimizer stage runs, visits every item, and changes nothing** after any ordinary router run. Both stages share one stop flag and nothing lowers it; every ordinary exit from the pass loop raises `AUTO_ROUTER_ONLY` (#214's five arms), so `runOptimizationStage`'s `isStopRequested()` (ALL) lets the stage **start** while `autoroutePassesForOptimizingItem`'s `isStopAutoRouterRequested()` runs **zero** passes per item. Every item rips, measures worse, and is restored. The stage costs **one whole-board deep copy per item** and produces the board it was given. Measured: six `OPT-ITEM` lines, all `improved=false`, board shape identical. | Give the optimizer a **stage-scoped stop**: reset the flag in `runOptimizationStage`, or have `autoroutePassesForOptimizingItem` read a **stage** flag rather than the job's. **This is the single largest quality change in the catalogue — an entire optimization stage begins working.** It **must** land with #202 and it **changes the run time of every board**. |
| **#202** | `--max-items` calls `requestStop()` (**ALL**) where `--max-passes` calls `requestStopAutoRouter()` (**AUTO_ROUTER_ONLY**), and `runOptimizationStage` returns early on the former — so a `--max-items` run **skips the optimizer entirely** and writes an unoptimised board, while `--max-passes` optimises normally. The log line says "Stopping auto-router"; **the optimizer is not the auto-router.** | Have the `maxItems` site call `requestStopAutoRouter()`. **Worth nothing on its own until #227 lands.** **Keep the three-state stop** (`CancelToken` → `RouterStop`) — survey §9.1: it must not collapse to a bool. |
| **#213** | `getAutorouteItems` appends an item **once per qualifying net**, and the pass runner then runs a fresh `0..netCount()` walk per appearance — so a two-net item is routed **four times in one pass**, on net indices unrelated to the ones that qualified it, each against the board the previous one left. | Append **outside** the net loop and **carry the qualifying net with the item** (`Vec<(ItemId, NetIndex)>`). The roadmap deferred it as "a different program"; with parity gone it is simply correct. Latent on the corpus (no multi-net items), so its evidence is the directed test. **Composes with R1's sort** — the sort key is computed once per (item, net) pair. |
| **#215** | The `else if` that resets the stagnation counter for a fully-routed board hangs off the `currentPass >= 8` guard, so it fires on passes 1-7 — where the counter **cannot yet be incremented** — and never afterwards. The counter first reaches its ten-pass window at pass 17 and the global tracker fires at 18. | Move `AutorouteBatchLoop.java:509-517` **inside** the `>= 8` arm as a third branch of the score test, which is what its own comment describes. **Changes which passes reset the counter on every board that completes early → changes where long runs stop.** |
| **#230 + #267** | `job.currentPass` is written by **two** loops (router from 1, optimizer from 0) and the manifest reports **whichever wrote last** under a key naming the **autorouter** — so one optimizer pass overwrites a three-pass routing stage with `1`. And on a `maxPasses`-capped exit the field is one behind the loop's own local, which the final event carries, so **the two APIs disagree by one at the exact moment the run stops.** Confirmed at corpus scale by the benchmark report (`passes_completed` misreports 1 while the logs show 18–30 passes). | Give the two stages **separate fields**; fill `phases.optimizer.passes_completed` — **the key already exists and is always `{}`** (#254, Task 20). Land the field split here; Task 20 writes the manifest. |
| **#228 (+ #217 as a decision; #216/#225/#226 record-only)** | Five dead or unreachable arms in the same two files: the board-rank break whose limit **is** the history's own cap (`rank > 30` has no solution, and the constant's comment has the reason **backwards**); a `HashSet` whose only two readers are commented out but which is still allocated and cleared twice; an empty `if` with an impossible third conjunct; a user-fixed guard that cannot fire because `getConnectionItems` already filters non-routable items; and a **`-1` sentinel a real pass improvement can equal** (latent — needs a pass that drives a positive score to hard zero). | **#228 gets its own `bool`** instead of a magic double. **#217 is a product question, not a cleanup:** making the rank break reachable would *stop runs earlier*, so **either set the limit strictly below the cap with an A/B, or delete the branch** — decide here and record which. Default: **delete the branch** and record that enabling it is a separate product decision (survey §10.2: fixing #198's `getRank` stability does **not** make it reachable). #216/#225/#226 are deleted as dead code with no behaviour change. |

**Tests (named, binding).**
- `crates/fr-router/tests/batch_loop.rs::a_normal_finish_reports_finished` — replaces `a_normal_finish_reports_cancelled_not_finished`; asserts each of `BatchLoopExit`'s five values on a constructed exit.
- `crates/fr-router/tests/optimizer.rs::an_auto_router_only_stop_still_runs_the_optimizer` — replaces `an_auto_router_only_stop_leaves_every_item_rejected`; asserts ≥ 1 `improved=true` on a board the router leaves improvable, and that the board shape **changes**.
- `crates/fr-router/tests/optimizer.rs::the_stage_scoped_stop_does_not_leak_into_the_router` — the three-state distinction preserved.
- `crates/fr-router/tests/stop_and_progress.rs::max_items_optimises_like_max_passes` — #202, observable only after #227.
- `crates/fr-router/tests/pass_runner.rs::a_two_net_item_is_routed_once_per_qualifying_net` — replaces `a_two_item_is_routed_four_times`; the literal is **2**, not 4.
- `crates/fr-router/tests/batch_loop.rs::the_stagnation_counter_resets_only_from_pass_eight` — asserts the reset fires at pass 8+, not 1-7.
- `crates/fr-core/tests/manifest.rs::the_two_stages_report_their_own_pass_counts` — a three-pass router stage and a one-pass optimizer stage read back as 3 and 1.
- `crates/fr-router/tests/optimizer.rs::the_improvement_flag_is_a_bool` — #228.

**Evidence / acceptance.** G1 green. **B, C, R move on every routed stem** — the optimizer now does work, so **every** board's geometry changes and **every** board's `cpu_s` rises. `goldens moved:` names #227 first and quantifies: trace length and via count per stem, before and after. **Direction check: normalized score must RISE on every stem** — an optimizer that runs and makes a board worse is a stop-and-report. `p7t8`, `p7t9`, `p8t1`, `p8t2` re-golden; `p7t9`'s RESULT line changes with #214 and the change is named. G2: score up on every stem; incompletes not up on any. **Timing (rulings BO/BP4) — Task 9 is the plan's largest speed exposure and is named in advance as expected to escalate.** #227 makes an entire optimizer stage begin working at **one whole-board deep copy per item**, so `cpu_s` rises on every stem; that is a **quality-winning** change and is tolerated, **but it is gated, not merely reported**: the `cpu_s` column is measured against `benchmark/baselines/stem-times.tsv` (Task 8's numbers), and **> 2× on any stem** or **> 20 % on the corpus median** raises `ESCALATE` and goes to the controller for an explicit keep/rework ruling **before the task closes** — pre-authorised here, with the expected shape stated so the escalation is a measurement, not a surprise. An earlier draft's "wall time is reported and explicitly not optimised" is **struck**: time is no longer outside acceptance, it is simply second in priority behind the hard routing metrics. #217's decision recorded in the register row.

**Steps:**
- [ ] Commit 1: #214 — `BatchLoopExit` out of the loop.
- [ ] Commit 2: #227 — the stage-scoped stop. **The plan's largest single quality commit.**
- [ ] Commit 3: #202 — `requestStopAutoRouter` at the `maxItems` site.
- [ ] Commit 4: #213 — `Vec<(ItemId, NetIndex)>`.
- [ ] Commit 5: #215 — the reset moves inside the `>= 8` arm.
- [ ] Commit 6: #230 + #267 — the two fields split.
- [ ] Commit 7: #228 + the #217 decision + the three dead-arm deletions.
- [ ] Commit 8: regenerate B/R/C; `goldens moved:` with the per-stem score/length/via table; the `cpu_s` column against Task 8's `stem-times.tsv`, with the escalation raised if it trips BO's thresholds.
- [ ] Register: seven rows → `fixed: T9`; survey §9.1's "#202's three-state stop" row stays `keep`.

**Commit message:** `feat(router): the optimizer stage begins working — #214 the exit reason, #227 a stage-scoped stop, #202, #213, #215, #230+#267, #228`

---

### Task 10: shove, obstacle and clearance decisions — survey group 10

**Controller note (Plan 9 Task 17, ruling BY): register row #297 owed.** Task 17's #193
instrumentation found that `MazeSearchEngine.reduceTraceShapesAtTiePins` — the maze search's
*only* board write — never fires on any of the eight batch stems (`autoroute/maze/MazeSearchEngine.java:157-162`;
port at `crates/fr-router/src/autoroute/maze/search.rs::reduce_trace_shapes_at_tie_pins`). This
is not a #193 question (it does not bear on the stale-index guards), and it is not in this
task's eight-row fix list; it is an open investigation with its own row, **#297**, `candidate`,
owned by **this task**. Before this task's #50/#179/etc. land, build a directed fixture with a
genuine multi-net tie pin (a pin on two nets) contacting a foreign-net trace and confirm whether
the predicate fires: if it does, corpus sparsity explains the zero and the row can close as
documented; if it does not, `is_tie_pin`/`is_foreign_trace` are mis-ported against `:157-162` and
need a real fix. Report the answer in this task's report and flip #297's status accordingly.

**Survey §10.1 constraint 15: #231 early, once.** It changes the clearance every committed reference was generated with, so doing it late invalidates every A/B taken before it. It is the first commit of this task. **Recommendation 3 is adopted**: make the option **continuous** and **keep 500 µm as the default**, then measure removing it as its own experiment — changing the semantics and the default in one commit would make the regeneration unreadable.

**Files:** `crates/fr-board/src/board/{shape_trace_entries.rs,trace_normalize.rs,clearance_override.rs,mod.rs}`; `crates/fr-router/src/{shove/trace_shover.rs,autoroute/maze_expand.rs,pipeline/run.rs}`; `crates/fr-board/src/{layer_structure.rs,items/conduction_area.rs}`; `crates/fr-router/src/board_ext/drill_item_mover.rs`; `crates/fr-router/tests/{fixtures.rs,maze_expand.rs,board_ext.rs}`; `crates/fr-board/tests/*`; a **new fixture with a signal-layer pour and a foreign-net trace**.

**Interfaces consumed:** `fr_board::Board::{change_conduction_is_obstacle, apply_copper_to_edge_clearance_override}` (Plans 2, 7), `fr_router::pipeline::prepare_board` (Plan 7).
**Interfaces produced:** a **continuous** copper-to-edge clearance option; `LayerStructure::get_signal_layer` returning `Option`.

**The fix list (8 rows).**

| # | mechanism | fix sketch |
|---|---|---|
| **#231 (+ #233)** | `applyCopperToEdgeClearanceOverride`'s guard makes the **default** `router.copper_to_edge_clearance_um` the one value that can be ignored: `=500` leaves a board with an explicit outline class alone while `=500.000001` or `=0` rewrites a whole clearance row and column and re-points the outline. On **15 of 16** corpus boards the guard cannot fire, so the plain `-de/-do` run every reference is generated from **does** carry a 500 µm board-edge keep-out. It reaches the output bytes (**15 254 B vs 14 644 B** on one stem) and is logged only at `debug`. #233: Java's own `TestingSettings` constructor **zeroes** the value, so **the Java suite measures a board no real run produces**. | Make the option **continuous**: apply the configured value uniformly and **drop the equals-the-default special case**; key "leave an explicit class alone" on **whether a source actually supplied the value** (the ladder knows — the field is `null` until one does), not on numeric equality. **Log at `info`.** Keep **500 µm as the default** (recommendation 3) and record a separate follow-up experiment for removing it. Note `crates/fr-router/tests/fixtures.rs` passes `=0.0` deliberately — that call site is updated and the reason re-worded. |
| **#65** | `ShapeTraceEntries.storeItems`' precedence bug (`&&` binds tighter than `\|\|`) skips a `ComponentObstacleArea` **unconditionally**, so **a component keepout can never block a via placement.** | `!isPadCheck && (a \|\| b)`. For a KiCad user this is the difference between a via landing inside a courtyard keepout and not. |
| **#69** | `storeTrace`'s three-way block test compares `contactItem.clearanceClassIndex() != contactTrace.clearanceClassIndex()` where `contactItem` **is** `contactTrace` — always false. **A contact whose clearance class differs never blocks a shove**, so the router shoves copper across a clearance-class boundary. | Third disjunct becomes `trace.clearanceClassIndex()` (symmetry with the second gives the intent). New directed test: **two contacting traces in different clearance classes.** |
| **#72** | `splitInsideDrillPadProhibited`'s precedence bug tests the `lastCorner` half for **this** trace too, and for a foreign trace whose first corner did not match — either answers "split allowed" even when a pad was found. **So a trace may be cut inside a pin pad.** | `currentTrace != this && (first \|\| last)`. Directed test: a trace crossing a pin pad. |
| **#174** | `TraceShover.check`'s via arm returns `false` **without setting `shoveFailingObstacle`**, where every other refusal records the culprit — so `MazeRipupResolver` is handed a **stale item, possibly from a different `check` call on a different net**. The field is **never cleared on entry**, so on a fresh board it can be null. **The router rips the wrong copper today.** | Set it to `currentShoveVia` **and clear the field on entry** — the register mentions the staleness but does not make it a fix; it should. `p6t9` mode `inst`'s `failing=` column carries the leftovers and is the before/after evidence. |
| **#179** | `checkNeckDownAtDestPin` **never asks whether the pin is a destination pin** and returns on the **first** `Pin` target door the room lists, start pin or not — so **a start pin's neckdown silently shrinks the trace the search plans through a room it is only passing through.** | Test `isDestinationDoor()` **inside** the loop and `continue` rather than `return`. Then the name, the javadoc and the body agree. **Read together with R2** — the other half of "the router necks down where it should not". |
| **#50** | `changeConductionIsObstacle`'s guard is `if (getIgnoreConduction() != value) return;` — it only does anything when the two are already **out of step** — and it ends by storing the **negation** of what it just wrote into every signal-layer conduction area, so the flag is a **latch that alternates** rather than a mirror. Non-signal-layer areas are skipped, so a power plane's `isObstacle` never changes. **This flag decides whether copper pours obstruct foreign-net routing** — the most user-visible boolean on a KiCad board with ground pours. | **Decide what the flag means and make it mean it**: guard `==`, store `value`. Needs a **new fixture with a signal-layer pour and a foreign-net trace**. |
| **#35, #46, #175** | `LayerStructure.getSignalLayer(n)` out of range returns the **last layer of the whole stack**, signal or not; `ConductionArea.copy` returns **`null`** whenever `netCount() != 1`, **including zero**, where every other `Item.copy` produces an item; `DrillItemMover.check` — a *check* — **mutates its caller's** `ignoreItems` and the recursion re-enters. | Return `Option`; implement at least the **zero-net copy** (it needs no new logic); **copy the collection unconditionally**, as `shoveVias` already does. |

**The three neckdown defects compound and must be measured together** (survey §10.2): **R2** (Task 2, the fanout fallback ignores the minimum width), **#179** (here) and **#51** (Task 14, a via that changed side keeps the old padstack's minimum width for ever, and `minWidth()` feeds the neckdown decision). Task 14's G2 report carries a joint neckdown column so none of the three is credited with the others' gain.

**Tests (named, binding).**
- `crates/fr-router/tests/fixtures.rs::the_copper_to_edge_override_is_continuous` — `=500`, `=500.000001` and `=0` all behave the same way; the explicit-class case keys on **provenance**, asserted by a source that supplies the value and one that does not.
- `crates/fr-board/tests/clearance_override.rs::an_explicitly_supplied_default_is_applied` — the guard's inversion, with the **15 254 / 14 644** byte figures re-measured and pasted.
- `crates/fr-board/tests/shape_trace_entries.rs::a_component_keepout_blocks_a_via` — #65.
- `crates/fr-board/tests/shape_trace_entries.rs::a_contact_in_another_clearance_class_blocks_the_shove` — #69, two contacting traces in different classes.
- `crates/fr-board/tests/trace_normalize.rs::a_trace_is_not_cut_inside_a_pin_pad` — #72.
- `crates/fr-router/tests/shove.rs::a_failed_via_shove_names_its_own_obstacle` and `…::the_failing_obstacle_is_cleared_on_entry` — #174.
- `crates/fr-router/tests/maze_expand.rs::a_start_pin_neckdown_does_not_shrink_a_pass_through_trace` — #179.
- `crates/fr-board/tests/conduction.rs::a_signal_layer_pour_obstructs_a_foreign_net` — #50, on the new fixture, with the hand-computed expectation reviewed here.
- `crates/fr-board/tests/layer_structure.rs::an_out_of_range_signal_layer_is_none`, `…::a_zero_net_conduction_area_copies`, `crates/fr-router/tests/board_ext.rs::drill_item_mover_check_does_not_mutate_its_caller` — #35/#46/#175.

**Evidence / acceptance.** G1 green. **B, C, R move on nearly every stem** (#231 alone reaches the output bytes of 15 of 16 corpus boards). `goldens moved:` names #231 first with the byte deltas, then #65/#69/#72/#174/#179 with their per-stem effects. G2: **violations must stay 0** (#65's keepout and #69's clearance-class boundary are both violation sources today) and incompletes must not rise. **The neckdown column is owed to Task 14** (ruling BP15): #179's per-stem `neckdown_below_class_width` count is committed in `benchmark/baselines/ab/quality-ab-T10.tsv`. The recorded follow-up experiment — "does the 500 µm board-edge keep-out apply by default at all?" — is written into the register row as an open question, **measured as a stem A/B at Task 16** (ruling BP3 demoted it from a bench arm; see there).

**Steps:**
- [ ] Commit 1: **#231 + #233**, first, alone, with its own regeneration and byte deltas.
- [ ] Commit 2: #65. Commit 3: #69. Commit 4: #72. Commit 5: #174.
- [ ] Commit 6: #179 (and the joint neckdown note pointing at Task 14).
- [ ] Commit 7: the signal-layer-pour fixture + #50.
- [ ] Commit 8: #35, #46, #175.
- [ ] Commit 9: regenerate B/R/C; `goldens moved:`.
- [ ] Register: eight rows → `fixed: T10`.

**Commit message:** `fix(board,router): shove, obstacle and clearance decisions — #231 continuous copper-to-edge, #65, #69, #72, #174, #179, #50, #35/#46/#175`

---

### Task 11: board geometry corrections — survey group 11

**Survey §10.1 constraint 7: #5 → #7 + #68.** The shove entry *point* must be right before the entry *side* is. **#9 is not fixed here** — its horizontal-`circleCenter` case lands with #82 in Task 13; this task fixes the other three of #15/#16/#9/#13 and leaves #9's arm to Task 13, with the split named at both ends.

**Files:** `crates/fr-geometry/src/{rational_point.rs,int_box.rs,int_octagon.rs,polyline.rs,polygon_path.rs,simplex.rs,circle.rs,line_segment.rs,tile_shape.rs}`; `crates/fr-board/src/{board/outline.rs,items/{obstacle_area.rs,component_outline.rs},component.rs}`; `crates/fr-router/src/{shove/trace_shover.rs,autoroute/path/inserter.rs,tightener/any_angle.rs}`; `crates/fr-geometry/tests/*`; `crates/fr-board/tests/areas_and_outlines.rs`; `crates/fr-router/tests/{forced_via.rs,inserter.rs,tightener.rs}`; a **new fixture with per-layer trace widths** (also #128's, Task 21).

**Interfaces consumed:** `fr_geometry::{FloatLine, FloatPoint, Polyline, Simplex, IntBox, IntOctagon}` (Plan 1), `fr_board::BoardOutline` (Plan 2).
**Interfaces produced:** `IntBox::border_line_index` / `IntOctagon::border_line_index` **implemented geometrically** (they were stubs returning `-1`); `PolygonShape::area` corrected (its stub half is Task 12's).

**The fix list (10 rows).**

| # | mechanism | fix sketch |
|---|---|---|
| **#5** | `RationalPoint.perpendicularProjection` uses `add` where `IntPoint` uses `subtract` — a **sign bug** giving a wrong projection for **any line not through the origin**, reachable from `ShapeTraceEntries` via `TileShape.nearestBorderPoint`, i.e. **the shove entry path**. | **One character.** The highest value-per-character fix in the register. **Land it before #7/#68.** |
| **#7 + #68** | `IntBox`/`IntOctagon.borderLineIndex` are **stubs that log and return `-1`**, and the live caller `ShapeAndEntrySide` receives it; in the same file both dog-ear cuts are guarded by an **always-true reference comparison**, so a cut that removed nothing still sets `cutOffAtStart`/`cutOffAtEnd` and the `fromSide` search hunts for a border line that is not there. **One fix** — #68's search is exactly what #7's `-1` breaks; the register lists them separately and does not connect them. | Implement `borderLineIndex` **geometrically against `borderLine(i)`**; compare the shapes **by value**. `p2t11` mode 5 pins all four combinations and is the before/after evidence. |
| **#177** | `TraceShover.insert` dereferences `board.changedArea` with **no null check** and its own `catch` **hides the NPE**, so on a board not marking its changed area the substitute traces are inserted **un-normalized** — JVM-pinned: **three traces where a marked board leaves one.** `ForcedPadRouter.forcedPad` — the same loop — **guards the identical call**, so the two mutating halves of the shove disagree about the same board state. | Compute `optArea` **the way `forcedPad` already does**, and **narrow the `catch`**. |
| **#186** | `FoundConnectionInserter` hands `connectToTrace` a `Trace` **the insert has already split away**, so the stub is inserted against a polyline the board no longer holds and the two tail removals then **delete both halves of the split trace.** Measured: both halves of trace 4 gone, its line surviving only inside a combined trace. | **Look the trace up by id after the insert.** The most visible geometry change in the register: it changes what the board looks like where a connection lands mid-trace. |
| **#187** | Each `connectToTrace` stub is sized from the **other** end's layer — the stub onto the target trace is inserted on the target's layer and sized from the *start* layer's `traceHalfWidth`. **There is no reading under which the width belongs to the layer the copper lands on.** Latent only because every corpus board shares a width across layers. | Let `connectToTrace` take the width **for the layer it has just computed**. Needs the **per-layer-trace-width fixture** — built here, reused by #128 in Task 21. |
| **#55** | All four `BoardOutline` transforms assign to the **loop variable** of an enhanced `for`, so `this.shapes` is **never written**: **the outline does not move, turn, rotate or mirror.** Only the lazily-built keepout follows, so the outline's curves and its outside-keepout disagree, and `boundingBox`/`lineCount`/`getShape` and the tree line bands keep answering from the untransformed shapes. | **Write back into the array.** *A board whose outline finally moves has a different routable region*, so the corpus re-baselines. |
| **#183** | `TraceTightenerAnyAngle.smoothenEndCornerAtTrace` reads `prevLineDirection` from the **same** line as `lineDirection`, so the `bend` arm needs two directions that two equal directions cannot satisfy — **the whole `bend` branch of the any-angle end-corner smoothener is unreachable.** | Read `lines[endLineNo - 1]`. Any-angle boards only, but **every end corner on them**. |
| **#48 + #57** | For a back-side item under `flipStyleRotateFirst`, `Component.rotate` adds `360 - angle` to the stored rotation and rotates the **location** by `angle`, so rotation and location disagree by `360 - 2·angle`; `ObstacleArea.rotateApprox` and `ComponentOutline.rotateApprox` have the **identical** split. **The component's outline and its pads disagree after a back-side rotation.** | **Decide which angle is intended and use it in all three at once** — fixing one makes them disagree instead. |
| **#15, #16, #13** (**#9's horizontal case → Task 13**) | `indexOfNearestCorner` seeds with `Double.MIN_VALUE` (the smallest **subnormal**), so a corner at distance **exactly 0** is never nearest; `nearestBorderPointsApprox`' upward insertion shift **copies the wrong element**; `LineSegment.stairApproximation45` calls **a function of *x* with a *y*-coordinate**. | `Double::MAX`; fix the shift and **check the `count > 1` callers**; use `functionInYValueApprox` and **verify against 45° output**. #9's `FloatPoint.circleCenter` divide-by-zero on horizontal input (giving `(x, NaN)`) **lands with #82 in Task 13** — it is the mechanism of #82 and must be measured with it. |
| **#26, #88, #23, #11, #17, #18, #32, #188** | The long tail: `PolygonShape.area()` **always returns 0** (its guard is `<= 2` where `dimension()` never exceeds 2) and is reached from `DsnFile`, so **plane autoroute settings derive from a zero board area**; `PolygonPath.boundingBox` adds `+ offset` to the running maximum on every **even** index, growing the upper x bound by `width/2` **per coordinate**; `Polyline(Point,Point)` repeats the start's closing direction, giving the **opposite** closing line from `Polyline(Polygon)`; `Simplex.cutoutFrom`'s `prevDivisionLine` is never assigned so **both merge branches are dead**; `IntOctagon.contains(FloatPoint)` is **inclusive** on the border where `IntBox`'s is exclusive; `IntBox.divideIntoSections` skips the base class's `dimension()==2` filter; `Circle.translateBy(RationalVector)` returns **`this` unchanged** where every sibling throws; `new Polyline(Line[])` **normalises the caller's array in place.** | As the register's columns state. **#26 additionally needs `corners[len-2]` guarded for a 1-corner polygon.** #26 (with #93, already landed in Task 4) **changes derived board-level numbers**, so it re-baselines more than a unit test. |

**Tests (named, binding).**
- `crates/fr-geometry/tests/rational_point.rs::perpendicular_projection_agrees_with_int_point` — #5, over a generated line set not through the origin.
- `crates/fr-geometry/tests/int_box.rs::border_line_index_is_geometric` and `crates/fr-geometry/tests/int_octagon.rs::border_line_index_is_geometric`; `crates/fr-router/tests/shove.rs::a_cut_that_removed_nothing_does_not_set_cut_off` — #7 + #68.
- `crates/fr-router/tests/forced_via.rs::an_unmarked_changed_area_still_normalises` — replaces the JVM-pinned "three traces" literal with **one**.
- `crates/fr-router/tests/inserter.rs::connect_to_trace_looks_the_trace_up_by_id` — #186; the existing test already fails today on the id lookup and the post-loop snapshot, **which is the fixed behaviour**.
- `crates/fr-router/tests/inserter.rs::the_stub_takes_the_width_of_the_layer_it_lands_on` — #187, on the new per-layer-width fixture.
- `crates/fr-board/tests/areas_and_outlines.rs::the_outline_moves_turns_rotates_and_mirrors` — four assertions, replacing the four pinning tests.
- `crates/fr-router/tests/tightener.rs::the_any_angle_bend_branch_is_reachable` — #183.
- `crates/fr-board/tests/component.rs::a_back_side_rotation_keeps_the_outline_and_the_pads_together` — #48 + #57, all three sites in one assertion.
- `crates/fr-geometry/tests/{shape.rs,line_segment.rs}::nearest_corner_at_distance_zero_is_nearest`, `…::the_insertion_shift_copies_the_right_element`, `…::stair_approximation_45_uses_the_y_function`.
- Eight named inversions for the #26 tail, one per site, including `polygon_shape.rs::area_is_not_always_zero` with a hand-computed area and `polyline.rs::the_caller_s_array_is_not_normalised_in_place`.

**Evidence / acceptance.** G1 green. **B, R move** via #5/#7+#68/#177/#186 (the shove and insert paths) and **G moves** via #55/#26 (the outline and the derived board area). `goldens moved:` names each. `p2t11` re-golden (modes 5 and 11). G2: incompletes must not rise; **#186's geometry change is the one to watch** — the survey calls it "the most visible geometry change in the register", so its per-stem diff is reviewed by eye on one stem and the review is recorded.

**Steps:**
- [ ] Commit 1: #5. Commit 2: #7 + #68. Commit 3: #177. Commit 4: #186.
- [ ] Commit 5: the per-layer-trace-width fixture + #187.
- [ ] Commit 6: #55. Commit 7: #183. Commit 8: #48 + #57.
- [ ] Commit 9: #15, #16, #13 (with #9 explicitly deferred to Task 13 and the deferral commented at the site).
- [ ] Commit 10: the eight-row #26 tail.
- [ ] Commit 11: regenerate G/B/R; `goldens moved:`; the eyeball review of #186 recorded.
- [ ] Register: ten rows → `fixed: T11`; #9's row stays `pinned` with "Task 13" in its status note.

**Commit message:** `fix(geometry,board): the finished board's geometry — #5, #7+#68, #177, #186, #187, #55, #183, #48+#57, #15/#16/#13 and the #26 tail`

---

### Task 12: polygon and circle implementations — survey group 12

**A workstream, not a correction.** These are **implementations**: five methods that are stubs today and one that recurses to a stack overflow. They get their own directed geometry suite with hand-computed expectations (recommendation 9), because there is no jar answer worth copying — the jar's answer is `null`, `0` or a crash.

**Files:** `crates/fr-geometry/src/{polygon_shape.rs,circle.rs}`; `crates/fr-geometry/tests/{polygon_shape.rs,circle.rs,stubs.rs}` (`stubs_match_the_java_stubs` is **deleted** here and its deletion named).

**Interfaces consumed:** `fr_geometry::{ConvexShape, TileShape, FloatPoint}` (Plan 1), `PolygonShape::area` (Task 11's #26 half).
**Interfaces produced:** `PolygonShape::{contains_on_border, cutout, enlarge, border_distance, distance, intersects_polygon, smallest_radius}`; `Circle::{nearest_point_approx, cutout}`.

**The fix list (2 rows).**

| # | mechanism | fix sketch |
|---|---|---|
| **#28 + #29** | `PolygonShape.containsOnBorder` is a **stub returning `false`**, so `containsInside(p) == contains(p)` for every polygon; `cutout`, `enlarge`, `borderDistance`, `distance` and `Circle.nearestPointApprox`/`cutout` are **unimplemented stubs returning `null`/`0`**, so **`smallestRadius()` always answers 0 for a polygon** — and that feeds the clearance heuristics. **KiCad exports polygon keepouts and zones routinely.** | **Implementations, not corrections** — their own workstream with their own directed geometry tests. **Every polygon keepout's clearance behaviour changes.** Pairs with #27 and with #26 (Task 11), which are the same class's other two holes. |
| **#27** | `PolygonShape.intersects(Shape)` binds to **itself** for polygon-vs-polygon and recurses to `StackOverflowError` — **not recoverable in either language**, because the handlers catch `Exception`, not `Throwable`. **Polygon keepouts are ordinary in KiCad exports.** | Add an `intersects(PolygonShape)` overload that **splits both sides to convex**; assert against a **hand-computed** answer. **Not the type test.** Upstream-PR candidate (recommendation 8's list). |

**Tests (named, binding).** A new `crates/fr-geometry/tests/polygon_geometry.rs`, every expectation hand-computed and reviewed **in this task's report**:
- `contains_on_border_distinguishes_inside_from_on_the_edge` — a square, a point on an edge, a point at a vertex, a point strictly inside.
- `smallest_radius_of_a_polygon_is_not_zero` — an L-shape whose inradius is a known value.
- `cutout_of_a_square_by_a_square_yields_the_expected_convex_pieces` — piece count **and** area sum.
- `enlarge_grows_every_edge_by_the_offset` — area check against the analytic formula.
- `border_distance_and_distance_agree_outside_and_differ_inside`.
- `circle_nearest_point_approx_lands_on_the_circumference` and `circle_cutout_of_a_box_has_the_expected_area` (numeric tolerance stated in the test, not inferred).
- `polygon_against_polygon_intersects_without_recursing` — replaces `polygon_against_polygon_reproduces_the_java_stack_overflow`; four cases: disjoint, touching, overlapping, one containing the other.
- `crates/fr-geometry/tests/stubs.rs` is **deleted** — the stubs are gone, and a test asserting they match Java's stubs is a test asserting the port is broken.

**Evidence / acceptance.** G1 green. **G moves, and B likely** — polygon keepout clearance behaviour changes on any board carrying one. `goldens moved:` names #28+#29 and lists the corpus boards with a polygon keepout (a grep over the fixture corpus, count pasted). **Timing (rulings BO/BP4) — Task 12 is one of the four tasks named in advance as likely to escalate**: seven constant-time `false`/`0`/`null` stubs become real geometry on **every** polygon keepout, which is pure added cost. The `cpu_s` column is gated against Task 16's `stem-times.tsv` (Task 12 dispatches after Task 16) with BO's thresholds and a pre-authorised escalation. G2: **violations must stay 0** and incompletes must not rise; a polygon keepout that finally has a non-zero `smallestRadius` makes the router avoid copper it used to route through, so **a small incomplete rise on one stem is a stop-and-report**, not a tolerance — if it happens, the keepout was being ignored and the honest answer needs recording the way #82's does.

**Steps:**
- [ ] Commit 1: the directed geometry suite with its hand-computed expectations, all failing.
- [ ] Commit 2: #28 + #29 — the seven implementations.
- [ ] Commit 3: #27 — the polygon-vs-polygon overload.
- [ ] Commit 4: delete `stubs_match_the_java_stubs`; regenerate G/B if moved; `goldens moved:`.
- [ ] Register: two rows → `fixed: T12`; #27 flagged **upstream-PR**.

**Commit message:** `feat(geometry): the polygon and circle holes are implemented — #28+#29 seven stubs, #27 polygon-vs-polygon intersection`

---

### Task 13: airlines, incompletes and board history — survey group 13

**Survey §10.1 constraint 8: #82 + #9 with #147, one commit, one `AIRLINE_BUDGETS` regeneration**, reading **every number that grew** — the register's own standing instruction for a deliberate ratsnest change. Survey §10.2: **#147 belongs with #82, not with #146** (Task 19).

**Files:** `crates/fr-board/src/datastructures/delaunay.rs` (the in-circle predicate and the bounding triangle, and its **private `JavaRandom` copy** at `:81` — **Task 19 edits the same file's `validate()` (#81) six tasks later, and Task 24 deletes the PRNG; Task 13 lands first and touches neither, ruling BP6**); `crates/fr-geometry/src/float_point.rs` (`circle_center` — #9's horizontal case, deferred here from Task 11); `crates/fr-drc/src/net_incompletes.rs`; `crates/fr-drc/src/airline.rs`; `crates/fr-router/src/pipeline/board_history.rs`; `crates/fr-router/src/score/statistics.rs` (**the fanout block only** — **Task 19's #195/#196 rewrite the length-breakdown and bounding-box blocks of this same file, and Task 20's #291 the drill count; dispatch order is 13 → 19 → 20, so each later task re-reads the file rather than assuming Task 13's line numbers, ruling BP6**); `crates/fr-router/tests/{board_history.rs,score.rs}`; `crates/fr-drc/tests/net_incompletes.rs`; `tests/reference/drc-*/AIRLINE_BUDGETS`; a **new multi-net SMD pin fixture**; `scripts/differential/run.sh` (`p2t13`'s Delaunay-order modes retire here).

**Interfaces consumed:** `fr_geometry::Point`'s `BigInt` machinery (the exact `sideOf` predicate already in the tree), `fr_board::datastructures::PlanarDelaunayTriangulation` (Plan 2).
**Interfaces produced:** an **exact** `in_circle` determinant; a `NetIncompletes::Edge` ordering that is **injective**; `BoardHistory::max_score` seeded `f32::NEG_INFINITY` and a **score-ordered** insertion.

**The fix list (5 rows).**

| # | mechanism | fix sketch |
|---|---|---|
| **#82 + #9** | Delaunay's bounding triangle is **finite with two corners exactly on the axes**, and every edge flip is decided by a float circumcentre that **silently degenerates to `NaN`** for three of six coordinate coincidences — answering "legal, do not flip". Ordinary axis-aligned input triggers it: **edges are lost at every grid size** (2×2 → 4/5, 5×5 → 50/56, 6×6 → 75/85), i.e. on **every pad row, column and BGA field**. Usually the MST just picks a longer airline, but on a 7×7 grid straddling the origin **~0.5 % of dense draws come apart and a witness pad gets no incident edge at all — so incompleteness is under-reported.** | An **exact `inCircle` determinant over `Point`** (the `BigInt` machinery `sideOf` already uses) and **a bounding triangle pushed out where no input can be collinear with it.** #9's horizontal `circleCenter` case is part of it — swap the point roles for the horizontal case. **Highest-value Tier-2 fix for a KiCad user**; re-baseline in the **same commit**, and **expect the honest incomplete count to rise.** Once the predicate is exact, **insertion order stops deciding the edge set**, which is what retires `delaunay.rs`'s private `JavaRandom` copy (Task 24 collects it; BL3 governs whatever survives). |
| **#147** | `NetIncompletes.Edge.compareTo` is **not injective** — its own comment says the four coordinate tie-breakers exist "so that edges with the same length are not skipped in the set", and **they do not achieve it**. Two candidate airlines between **different** items whose corners coincide pairwise compare `0` and `TreeSet.add` **drops the second**: a via stacked on a pad, or two pads at one location. `Signum.asInt` also maps **NaN** to 0. | Break the remaining tie on **the two `NetItem` indices** (already to hand at the construction site) and guard the NaN with `Double.compare` (`f64::total_cmp` in the port — see Task 23's `java_double_compare` row). **Same channel as #82; land together and regenerate `AIRLINE_BUDGETS` reading every number that grew.** |
| **#197 + #198** | `BoardHistory.getMaxScore` seeds with **`0`, not `-inf`**, so an empty history can never trigger a restore and a negatively-scored one only against a board below zero; `restoreBoard` **sorts the list in place under a *read* lock** and `getRank` reports the current position, so a board's rank is **insertion order until the first restore and score order after** — and the pass loop **breaks** when `getRank > BOARD_RANK_LIMIT`. **How many restores have happened changes when the router stops.** | Seed `f32::NEG_INFINITY`; **keep the list in score order at insertion** so `getRank` is stable. These two decide the **final output** of every multi-pass run. Note the rank break is #217's dead arm (Task 9): **fixing #198 does not make it reachable**; that was a separate decision, already taken. |
| **#194** | `BoardStatistics`' fanout block decides `pinsToEscape` from **net index 0 only** while `total` counts every SMD pin with `netCount() > 0` and `isPinEscaped` is **net-blind** — so a pin connected on its first net and unconnected on its second is counted as needing no escape and `BatchFanout` **skips it**. | **Loop the pin's net indices**, as the item loops elsewhere in the same class. Needs a **multi-net SMD pin fixture** (recommendation 9). |
| **#148** | `AirLine.compareTo` compares **the net name alone**, so a `TreeSet<AirLine>` would collapse a net's 29 airlines into one; also an NPE waiting on a null `net`. Latent — nothing in the Java tree sorts or set-collects `AirLine`s. | **Drop `Comparable` entirely**, which the port already effectively does — or compare name, then both item ids, then the corners. Default: drop it, and record that the ordering is not a contract. |

**Tests (named, binding).**
- `crates/fr-board/tests/delaunay.rs::a_square_pin_grid_keeps_every_edge` — replaces `square_pins_javas_four_edges`; asserts **5/5, 56/56, 85/85** at 2×2, 5×5 and 6×6.
- `crates/fr-board/tests/delaunay.rs::a_seven_by_seven_grid_straddling_the_origin_leaves_no_witness_pad_edgeless` — the 0.5 % case, over 2 000 dense draws, **0 failures**.
- `crates/fr-board/tests/delaunay.rs::the_edge_set_is_independent_of_insertion_order` — the property that retires the private PRNG; asserts two shuffles give the same edge set.
- `crates/fr-geometry/tests/float_point.rs::circle_center_of_a_horizontal_input_is_finite` — #9.
- `crates/fr-drc/tests/net_incompletes.rs::two_airlines_between_different_items_at_one_location_are_both_kept` and `…::a_nan_length_does_not_compare_equal` — #147; the three existing unit tests are re-pointed.
- `crates/fr-router/tests/board_history.rs::an_empty_history_can_trigger_a_restore`, `…::a_negatively_scored_history_restores`, `…::get_rank_is_score_order_before_the_first_restore` — the three existing tests, inverted.
- `crates/fr-router/tests/score.rs::a_pin_unconnected_on_its_second_net_needs_an_escape` — #194, on the new fixture.
- `crates/fr-drc/tests/net_incompletes.rs::airline_is_not_comparable` — #148.

**Evidence / acceptance.** G1 green. **D and C move** — incomplete counts change on every DRC stem and every manifest. **`AIRLINE_BUDGETS` is regenerated in the same commit as #82+#9+#147**, and **every number that grew is read and named** in the commit message, one line per stem. **The honest incomplete count is expected to RISE**, and that is the standing example of survey §7.4's rule: *a fix that makes the score worse and is still correct lands anyway, with the reason recorded.* G2 therefore **reports `incomplete_count` both ways on the same board** for this task and the next (BL2's rule) — the pre-#82 metric and the post-#82 metric side by side — so a later task's improvement is not confused with the metric moving underneath. **Timing (rulings BO/BP4) — Task 13 is one of the four tasks named in advance as likely to escalate**: an exact `BigInt` in-circle determinant replaces a float circumcentre in the hot ratsnest path, so the `cpu_s` column is gated against Task 11's `stem-times.tsv` with BO's thresholds and a pre-authorised escalation. **The G2 gate-version bumps to `g2` in this task** (ruling BP8): `incomplete_count` changes meaning here, so the same commit re-cuts Task 11's affected baseline columns under the new gate and commits both, and every later A/B compares within `g2`. `p2t13`'s Delaunay-order modes **RETIRE** (BL7): final MATCH count recorded, the pair deleted, the Rust half's edge-count assertions kept as unit tests. `p5t1`, `p5t2` re-golden; `p7t7` re-golden for #194.

**Steps:**
- [ ] Commit 1: the multi-net SMD pin fixture with its reviewed expectation.
- [ ] Commit 2: **#82 + #9 + #147 in one commit**, with the `AIRLINE_BUDGETS` regeneration and the grew-numbers list.
- [ ] Commit 3: #197 + #198.
- [ ] Commit 4: #194.
- [ ] Commit 5: #148.
- [ ] Commit 6: retire `p2t13`'s Delaunay modes; regenerate D/C; `goldens moved:` with the both-ways incomplete table.
- [ ] Register: five rows → `fixed: T13`; #9's Task-11 deferral note closed.

**Commit message:** `fix(drc,board): the ratsnest tells the truth — #82+#9 an exact inCircle, #147 an injective edge order, #197+#198 board history, #194, #148`

---

### Task 14: stale caches and via-rule identity — survey group 14

**#51 and #60 first** — they are the two that reach routing, and **#51 compounds R2 and #179** (survey §10.2: three independent ways the router necks down wrongly). This task's G2 report carries the **joint neckdown column** promised at Task 10.

**Files:** `crates/fr-board/src/items/{drill_item.rs,polyline_trace.rs,component_outline.rs}`; `crates/fr-board/src/board/mod.rs` (`set_flip_style_rotate_first`); `crates/fr-board/src/searchtree/shape_tree.rs` (`insert`); `crates/fr-board/src/rules/{via_rule.rs,via_info.rs}`; `crates/fr-router/src/board_ext/routing_board_ext.rs` (`combined_fallback_via_rule`); `crates/fr-board/tests/*`; `crates/fr-router/tests/fanout_order.rs`.

**Interfaces consumed:** `fr_board::{DrillItem, PolylineTrace, ShapeTree, ViaRule, ViaInfo}` (Plan 2).
**Interfaces produced:** `ViaInfo` with **value equality** on (name, padstack, clearance class, attach flag) — which **deletes one of the two readers of `fr_geometry::Line`'s identity token** (the other is #74, Task 18).

**The fix list (2 rows).**

| # | mechanism | fix sketch |
|---|---|---|
| **#51, #60, #56, #58, #41** | Five memos whose input moved behind them. **#51:** `DrillItem.clearDerivedData` resets two layer memos but **not `precalculatedMinWidth`**, so a via that changes side keeps the minimum pad width of a padstack it **no longer has, for the rest of its life** — and `minWidth()` feeds the autorouter's **neckdown** decisions. **#60:** `PolylineTrace.rotateApprox` alone of the four transforms does not `clearDerivedData()`, so a rotated trace keeps search-tree tile shapes for its **pre-rotation** position — *a stale tree shape is a wrong-answer obstacle.* **#56:** `ComponentOutline.clearDerivedData` does not chain to `super`. **#58:** `setFlipStyleRotateFirst` clears no caches. **#41:** `ShapeTree.insert` returns early for a zero-shape object **before telling it**, so a stale entry array survives an "insert". | Each is the **one-line addition** the register names. **#51 and #60 first** — they are the two that reach routing. |
| **#218** | `ViaRule.contains` is object **identity** (`ViaInfo` declares no `equals`), so `RoutingBoard.fanout`'s fallback merge appends a via **value-equal to one it already holds** whenever a `.rules` file re-declares a `(via …)`. The duplicate widens `viaInfos`, re-runs the `viaRadii` maximum and **doubles the entries the maze walks.** Java: `ViaRule.java:55-62`; `RoutingBoard.java:1026-1041`. Port: `ViaRule::contains` / `ViaInfo::is_same_object` (an identity serial, ruling AE's shape). | Give `ViaInfo` **value equality** on (name, padstack, clearance class, attach flag) and the dedup does what the code reads as. **Deletes an identity-token mechanism the port only carries for parity.** |

**Tests (named, binding).**
- `crates/fr-board/tests/drill_item.rs::a_via_that_changes_side_recomputes_its_minimum_width` — #51, with the neckdown consequence asserted (the width the router would neck down to changes).
- `crates/fr-board/tests/polyline_trace.rs::a_rotated_trace_clears_its_tree_shapes` — #60.
- `crates/fr-board/tests/component_outline.rs::clear_derived_data_chains_to_super` — #56.
- `crates/fr-board/tests/board.rs::setting_flip_style_clears_the_caches` — #58.
- `crates/fr-board/tests/shape_tree.rs::a_zero_shape_insert_still_clears_the_entry_array` — #41.
- `crates/fr-router/tests/fanout_order.rs::a_redeclared_via_rule_is_deduplicated_by_value` — #218; asserts `viaInfos.len()` and the `viaRadii` maximum both unchanged by the redeclaration.
- `crates/fr-board/tests/via_info.rs::via_info_equality_is_by_value` — the four fields, and the identity serial's removal.

**Evidence / acceptance.** G1 green. **R and B move via #51 and #60** (both reach routing); #56/#58/#41 are U-only; #218 is B-likely on rules-carrying boards. `goldens moved:` names #51 and #60 with their per-stem effects. **G2 carries the joint neckdown column**: for each stem, the number of traces below the class width, before R2 (Task 2), after R2, after #179 (Task 10) and after #51 (here) — so none of the three is credited with the others' gain. **The first three columns are READ, not reconstructed** (ruling BP15): Task 2 commits its pre-R2 and post-R2 rows and Task 10 its post-#179 row to `benchmark/baselines/ab/quality-ab-T{2,10}.tsv` under a `neckdown_below_class_width` column, and this task's table is a join over those committed tsvs plus its own — all four at the same `gate-version`. If a column is missing from the committed artefacts, that is a stop-and-report on the task that owed it, never a re-derivation from two-task-stale goldens. G2 rule unchanged: incompletes not up, violations 0.

**Steps:**
- [ ] Commit 1: #51. Commit 2: #60. (Each with its own A/B row.)
- [ ] Commit 3: #56, #58, #41.
- [ ] Commit 4: #218 — value equality; the identity serial deleted; the `Line` token's remaining reader (#74) named at the site.
- [ ] Commit 5: regenerate R/B; `goldens moved:`; the joint neckdown table.
- [ ] Register: two rows → `fixed: T14`; the ruling-AE row updated to "one reader left (#74)".

**Commit message:** `fix(board): stale caches and via-rule identity — #51+#60 the two that reach routing, #56/#58/#41, #218 value equality`

---

### Task 15: fanout ordering, stagnation and the ripped set — survey group 15

**Files:** `crates/fr-router/src/board_ext/routing_board_ext.rs` (`fanout`); `crates/fr-router/src/pipeline/fanout.rs` (the loop state, the gates, the comparator); `crates/fr-settings/src/router_settings.rs` (`pin_sorting_order` validation); `crates/fr-router/tests/{fanout.rs,fanout_order.rs}`.

**Interfaces consumed:** `fr_board::Board::structural_hash` (Plan 7, widened), `fr_router::pipeline::FanoutLoopState` (Plan 7).
**Interfaces produced:** `distance_to_closest_on_net: Option<f64>`; a **validated** `pin_sorting_order`; a fanout retry with its **own** ripped set.

**The fix list (3 rows).**

| # | mechanism | fix sketch |
|---|---|---|
| **#221** | `RoutingBoard.fanout`'s two-attempt strategy hands the retry **the same `rippedItemList`**, and `autorouteConnection` removes each ripped item's whole connection from the board — so **a successful retry destroys connections only the abandoned first attempt asked to rip.** Java: `RoutingBoard.java:1058, :1064-1085`. | Give the retry a **fresh set** and **union it into the caller's only on success.** **T1-adjacent: it deletes live copper.** The port's `fanout_order.rs` asserts the single binding today — the retry fires on none of the eight `p7t5` boards — so the evidence is the directed test plus the argument. |
| **#222** | The fanout loop's oscillation detector packs a pass into one `long` (`routedCount << 32 ^ viaCount`) — **a summary, not a board** — so two passes that escape *different* pins in different places read as "no progress" and the run can end **while it is still moving**. `getHash()`, the thing that actually answers the question, is taken **three lines later**. And the constant is a **repeat** count, so the break fires on the **fourth** identical pass while the log says three. | **Compare the board hash**; rename the constant and fix the message. |
| **#219 + #220 + #223** | Three fanout gates that answer arbitrarily: a pin alone on its net keeps **`Double.MAX_VALUE`** as its `distanceToClosestOnNet` and **that sentinel is a sort key** (two such pins tie and fall through to `pinIndex`); an **unrecognised** `pinSortingOrder` silently degrades the comparator to a **fifth, undocumented ordering with no warning**; and `canUseVias` hangs off `net != null`, so **a pin whose net number names no net skips the via check entirely** — the case the code knows least about is the one it does not check. | `Option<f64>` for "no other pin on this net", **sorted explicitly**; **validate `pinSortingOrder` in `RouterSettings.validate`** (or fall back to `outer_first` **with a warning**); treat an **unresolvable net as "cannot use vias"**. |

**Tests (named, binding).**
- `crates/fr-router/tests/fanout_order.rs::a_successful_retry_does_not_destroy_the_first_attempts_rips` — #221; a constructed two-attempt case where the first attempt rips a connection the second does not need.
- `crates/fr-router/tests/fanout.rs::two_passes_escaping_different_pins_are_not_oscillation` — #222, three pins (the existing test's shape), asserting the loop continues.
- `crates/fr-router/tests/fanout.rs::the_oscillation_break_fires_on_the_named_repeat_count` — the constant's rename and the message.
- `crates/fr-router/tests/fanout_order.rs::a_pin_alone_on_its_net_sorts_explicitly` — #219; asserts the ordering does not depend on `f64::MAX`.
- `crates/fr-settings/tests/router_settings.rs::an_unrecognised_pin_sorting_order_is_refused` — #220.
- `crates/fr-router/tests/fanout.rs::a_pin_whose_net_does_not_resolve_cannot_use_vias` — #223.

**Evidence / acceptance.** G1 green. **B and C move on the fanout stems** (`router-fanout-bm11`, `cli-router-fanout-bm11`). `goldens moved:` names #222 first (it changes where the fanout stage stops on every board that oscillates) and #219 second (it changes the escape order). `p7t5` re-golden. G2: incompletes must not rise; the fanout stems' escaped-pin counts are reported before and after.

**Steps:**
- [ ] Commit 1: #221 — the fresh ripped set, unioned on success.
- [ ] Commit 2: #222 — the board hash, the constant, the message.
- [ ] Commit 3: #219 + #220 + #223 — the three gates.
- [ ] Commit 4: regenerate B/C on the fanout stems; `goldens moved:`.
- [ ] Register: three rows → `fixed: T15`.

**Commit message:** `fix(router): fanout ordering, stagnation and the ripped set — #221 a fresh retry set, #222 the board hash, #219+#220+#223 the three gates`

---

### Task 16: the via optimizer and the drill pages — survey group 16 · **MILESTONE M2**

**Files:** `crates/fr-router/src/optimize/via_optimizer.rs`; `crates/fr-router/src/optimize/connection_to_pin.rs`; `crates/fr-router/src/autoroute/drill_page_array.rs`; `crates/fr-router/tests/{via_optimizer.rs,connection_to_pin.rs,maze_drills.rs}`.

**Interfaces consumed:** `fr_router::optimize::ViaOptimizer` (Plan 7), `fr_board::Board::get_normal_contacts` (Plan 2).
**Interfaces produced:** `is_within_tolerance` **deleted**; `reposition_via`'s angle-restriction test applied to both overloads.

**The fix list (2 rows).**

| # | mechanism | fix sketch |
|---|---|---|
| **#206 + #207 + #205** | Three `ViaOptimizer` rows. **#206:** `isWithinTolerance` is a **loose Manhattan** test standing in for an **exact** connectivity rule, tested `firstCorner` first — so a trace whose first corner is within a **37 821-unit** window of a via sitting exactly on its **last** corner reads the wrong end and `repositionVia` is aimed at the wrong corner. **#207:** **no** `repositionVia` overload tests the board's angle restriction against the delta it produces, though the projection fallback in the calling method does. **#205:** `checkConnectionToPin`/`correctConnectionToPin` accept `pinEdgeToTurnDist == 0` where their only caller demands `> 0` — a **dead acceptance band**. | **Compare exactly, as `getNormalContacts` does, and delete `isWithinTolerance`**; apply the delta test to **overload A's** answer too; make the three guards agree at `<= 0`. `p7t4` is the live diff and is the before/after evidence. |
| **#192** | `DrillPageArray.overlappingPages` mixes an **`int` lower bound with a `double` upper bound**, so a shape whose upper edge lands **exactly on a page boundary stops one page short** — a coverage *and* an ordering input, because **page ids order the maze queue**. | **Compute both bounds the same way**, and **decide deliberately** whether a boundary-touching shape reaches the next page. Default: it does (a shape touching a page overlaps it), and the decision is recorded in the register row. |

**Tests (named, binding).**
- `crates/fr-router/tests/via_optimizer.rs::a_via_on_the_last_corner_is_matched_to_the_last_corner` — #206, with the 37 821-unit window case asserted to be **rejected** now.
- `crates/fr-router/tests/via_optimizer.rs::reposition_via_respects_the_boards_angle_restriction` — #207, both overloads.
- `crates/fr-router/tests/connection_to_pin.rs::a_zero_pin_edge_to_turn_distance_is_refused` — #205, all three guards.
- `crates/fr-router/tests/maze_drills.rs::a_shape_ending_on_a_page_boundary_reaches_the_next_page` — #192, with the decision named in the test's doc comment.

**Evidence / acceptance.** G1 green. **R and B move**: #206 changes which corner a via is repositioned toward on every board with a via near a trace end, and #192 changes the drill-page coverage and therefore the maze queue order on every layer-changing connection. `goldens moved:` names both. `p7t4` re-golden.

**MILESTONE M2** — the G3 block **exactly as written**, with `--run-id plan9-m2`, compared `--runs java-278fe14,plan9-m2` into `benchmark/reports/plan9-m2-vs-java-278fe14.{json,md}` (`git add -f`), `remote-run.sh`'s output to `benchmark/baselines/plan9-m2.log`, and the workbench binary's sha pasted against `meta.json`'s. **Acceptance:** `overall.verdict` `better` or `same`, **zero hard losses**, against the frozen `java-278fe14`. **Against M1** the comparison is a second, cheap compare over runs already on disk — `uv run bench compare --baseline rs-main --against rs-main --runs plan9-m1,plan9-m2 --tier pcbench --out plan9-m2-vs-m1` is not expressible (one candidate name, two runs), so the M1→M2 delta is read from the two compare JSONs' per-board rows and reported as a table: boards that moved, in which direction, and the **corpus-median `cpu_s` ratio M2/M1** (BO/BP4). Additionally recorded, because M2 is the point where the T2 heart is complete:
- **The I2 (#296) via-inflation column**, per D3 subset, against M1 and against the `v1.0.0-rs` run of record. If the inflation has deflated with R1, the register row is **closed** with the measurement. If it survives, the row stays open and names **#172** (Task 18) as the next hypothesis — the survey's §10.2 reading is that the 10× via discount on pure-SMD nets and the inflation are plausibly the same phenomenon from two ends.
- **The #231 follow-up, as a STEM A/B — not a bench arm** (ruling BP3, correcting the earlier draft). An extra candidate arm is a **second full 605-board routing pass**, not a free rider on M2's run, and no candidate entry for a `copper_to_edge_clearance_um=0` build exists in any candidates file. So the "does the 500 µm default apply at all?" question is answered at **G2 scale**: `scripts/quality-ab.sh T16 --arm copper_to_edge_clearance_um=0` over the 29 stems, committed as `benchmark/baselines/ab/quality-ab-T16-cte0.tsv`, with its own `cpu_s` column. **The plan authorises no fourth corpus run.** If — and only if — the stem A/B shows a signal worth a corpus number, the task **stops and reports** and the controller decides whether to authorise one; that is a controller ruling, never a local decision.
- **The joint neckdown table** (R2 / #179 / #51) is Task 14's, read from the committed A/B tsvs (BP15); M2 reports its corpus-scale counterpart from the compare's per-board violation rows.

**Steps:**
- [ ] Commit 1: #206 + #207 + #205.
- [ ] Commit 2: #192, with the boundary decision recorded.
- [ ] Commit 3: regenerate R/B; `p7t4` re-golden; `goldens moved:`.
- [ ] Commit 4: the #231 stem A/B arm (`quality-ab.sh T16 --arm copper_to_edge_clearance_um=0`); its tsv committed; the answer written into the register row.
- [ ] Commit 5: **M2** — the workbench build + sha check, `remote-run.sh … --run-id plan9-m2`, the frozen-view compare; `git add -f benchmark/reports/plan9-m2-vs-java-278fe14.{json,md}`; the I2 column, the M1→M2 delta table and the cpu-ratio line.
- [ ] Register: two rows → `fixed: T16`; #296 closed or re-pointed at #172; #231's follow-up answered.

**Commit message:** `fix(router): the via optimizer and the drill pages — #206+#207+#205 exact matching, #192 the page bounds; M2 bench recorded`

---

### Task 17: #193 — the stale tree-index discovery — survey group 17

**Discovery first, code second.** The survey starts this early (its sequence runs it alongside groups 3–7) **because its answer may subsume several Task 8 rows**. This plan is a sequential writer (BL4), so Task 17 is **dispatched immediately after Task 7**, before Task 8, and its instrumented run is a **read-only measurement** — no behavioural change, no golden moves.

**Files:** `crates/fr-router/src/autoroute/engine.rs` (the three HEAD-only guards, instrumented); `crates/fr-router/src/autoroute/instrument.rs` (**new**, `#[cfg(feature = …)]`-free — a plain module gated by a `RouterCounters` field so it costs nothing when off); `docs/plan-9-prep/stale-index-report.md` (**new**, the deliverable); `crates/fr-router/tests/stale_index.rs`.

**Interfaces consumed:** `fr_router::pipeline::RouterCounters` (Plan 7).
**Interfaces produced:** a counter set on `RouterCounters` recording **which mutation invalidated which index**, and the report.

**The fix list (1 row).**

| # | mechanism | fix sketch |
|---|---|---|
| **#193** | Three HEAD-only guards that each admit **an item's tree-shape indices go stale while the search is running**; they silently `continue`, and one **resizes `expansionRoomArr` mid-search**. **The corpus reaches all three.** *The guards convert a corrupted search into a quietly worse route.* | **A discovery workstream, not a fix.** **First deliverable: an instrumented run recording *which* mutation invalidated *which* index, over the six router stems.** Until it is answered **the guards stay**. The report names, for each of the three guards: how often it fires per stem, which board mutation preceded it, and whether the stale index is recoverable (re-derivable from the item) or genuinely lost. |

**Tests (named, binding).**
- `crates/fr-router/tests/stale_index.rs::the_three_guards_are_counted_on_every_router_stem` — asserts the counters are non-zero on at least one stem (the survey's claim that the corpus reaches all three is **verified, not assumed**; if a guard never fires, that is the finding and the report says so).
- `crates/fr-router/tests/stale_index.rs::instrumentation_changes_no_board_byte` — the Plan 7 ruling 11 shape, re-run: with the counters on and off, `batch.ses` is byte-identical on all eight stems. **This is the gate that makes the task safe to land.**

**Evidence / acceptance.** G1 green. **No golden moves** — and that is the acceptance. The deliverable is `docs/plan-9-prep/stale-index-report.md` with, per guard per stem: fire count, the preceding mutation, and a recoverability verdict; plus a recommendation with three named options (re-derive the index at use; make the index a generation-checked handle; leave the guard and document it). **The recommendation is written into #193's register row and its status stays `pinned`** until a later decision acts on it — this task does not fix #193, and saying so is the point.

**Steps:**
- [ ] Commit 1: the instrumentation module and the two tests, with the byte-identity gate green.
- [ ] Commit 2: the run over the six router stems + eight batch stems; the report written.
- [ ] Commit 3: #193's register row updated with the findings and the recommendation; status stays `pinned`.

**Commit message:** `test(router): instrument the three stale tree-index guards — #193's discovery report, no behaviour change`

---

### Task 18: measured policy switches and orderings — survey group 18 · **MILESTONE M3**

**Last on purpose.** Every row here is a deliberate change of policy or of an arbitrary order, and each needs the rest of the catalogue's improvements underneath it before its A/B means anything. **Every row is individually abandonable**, and abandoning one is a recorded outcome, not a failure. **Recommendation 11 is adopted:** implement #182, #172 and #235 as **settings with the CURRENT behaviour as the default**, then decide each default from its own A/B.

**Files:** `crates/fr-router/src/tightener/{mod.rs,acid_traps.rs}`; `crates/fr-router/src/autoroute/control.rs` (`rebuild_via_info`); `crates/fr-router/src/pipeline/failure_log.rs`; `crates/fr-dsn/src/parser/network.rs` (`create_via_rule`); `crates/fr-router/src/tightener/tightener_45.rs`; `crates/fr-board/src/{ids.rs,items/mod.rs,board/mod.rs}` (`items_in_board_order`, the `.rev()`s); `crates/fr-geometry/src/line.rs` (the identity token); `crates/fr-settings/src/router_settings.rs` (three new settings); an **acid-trap fixture**.

**Interfaces consumed:** everything above.
**Interfaces produced:** `router.avoid_acid_traps` (default **off**), `router.smd_via_relaxation` (default **on**, i.e. today's behaviour), `router.failure_give_up_threshold` (default **disabled**, i.e. today's behaviour).

**The fix list (6 rows).**

| # | mechanism | fix sketch |
|---|---|---|
| **#182** | `TraceTightener.avoidAcidTraps` is **disabled by its own first statement** — `if (true) { return polyline; }` — so 20 lines of `springOverObstacles` + `checkPolylineTrace` are dead and all three `pullTight` overrides hand their argument back. **The port has no acid-trap avoidance at all, because Java does not.** | **Turning a feature on, not fixing a bug.** Give it its **own setting**, default it **on only after a measurement round** — nobody has ever run this code. Needs an **acid-trap fixture** (a trace approaching a same-net pin at an acute angle). **Deleting the dead body is NOT an equal alternative**: it removes the option. |
| **#172** | `AutorouteControl.rebuildViaInfo` relaxes **two** routing gates for a pure-SMD net, overriding what the padstacks say: it forces `attachSmdAllowed = true` while the per-via `ViaMask` keeps saying `false` (**the two disagree inside the same object**), and it multiplies `viaCostFactor` by **`0.1`**, making **every via ten times cheaper**. HEAD-only; JVM-pinned at `minNormalViaCost` **400 vs 4000**. | Make the relaxation an **explicit router setting** rather than an implicit override of the DSN. **A policy row: change the default, do not delete the behaviour.** **Interacts with I2 (#296)** — a 10× via discount on SMD nets is a plausible contributor to the via inflation, and M2 will have said whether I2 survived R1. |
| **#235** | `RoutingFailureLog`'s documented `FAILURE_THRESHOLD = 50` give-up policy **never runs**: `shouldSkip`'s one caller is on the dead multithreaded path and the other three methods have **no callers at all**. An item that fails 50 times is retried on the 51st pass exactly as on the first. Port: six `// not ported:` markers, one per caller-less member. | **A policy decision**: wiring `shouldSkip` into the item loop is a real behaviour change (**faster, possibly fewer connections**) and belongs **behind a setting with an A/B**, not in a cleanup. Otherwise delete the four methods and the constant. Default: **implement the setting, default disabled**, and let the A/B decide. |
| **#104** | `Network.createViaRule` takes an `attachAllowed` it **never reads**, so **a `(via_at_smd on)` control scope has no effect on a net class's `use_via` rule** — only on the via infos. | **Use the parameter.** Dropping it is behaviour-preserving and is **not** the fix (the register does not distinguish the two — the roadmap's correction). **Pairs with #172**: both decide whether SMD pads get vias. |
| **#210** | `TraceTightener45.smoothenStartCornerAtTrace` keeps the **last** matching contact of a set ordered by **descending item id**, so which contact shapes the new corner is decided by an id ordering with **no geometric meaning** (measured: contacts `{496, 495, 494}`, both 496 and 494 match, Java keeps 494). **Unlike #44/#63/#74 this one does have a correctness argument.** The port's `.rev()` at `tightener/mod.rs` is a port defect **already fixed to match Java** (ruling AY); the Java-side fix is still owed. | **Pick the contact on geometry** — nearest, or smallest `translateDist`. **Land it on its merits, independently of #44/#63.** Ruling AY's sibling audit already cleared the other three candidate sites. |
| **#44 + #63 + #74** | The three load-bearing orderings. **#44:** `Item.compareTo` subtracts the wrong way round, so every `TreeSet` the search tree returns — **the order the router visits overlapping items in** — is **descending id**. **#63:** `UndoableObjects` is keyed by that comparator, so every walk of the board's item list is descending, and that decides the **structure** of every search tree `MinAreaTree` builds. **#74:** `PolylineTrace.change` compares `Line`s by **object identity**, so a freshly built polyline always differs at index 0, the "no change necessary" early returns are near-unreachable, and `keepAtStart/EndCount` decides how many tree leaves are reused rather than re-inserted — a value comparison disagreed on **1 403 of 2 358** calls on one board. | **Measure-only, and the last thing to touch.** No correctness argument stands behind any of the three: **any total order is as valid**, and "fixing" them re-baselines every reference for **no predicted gain**. **#74 alone has a cost argument** (a value comparison reuses *more* leaves). Do them as **one deliberate "ordering flip" commit with its own A/B — or not at all.** `Line`'s identity token exists **only** for #74 and #218 (Task 14, landed); **closing both deletes it.** |

**The abandonment rule, stated once.** Each of the six is committed **separately**, measured at G2 **separately**, and carried into M3. **A row whose G2 shows a hard loss is reverted in the same task**, its register row is set back to `pinned` with the measurement recorded, and Task 25's report names it as *measured and declined*. **#44+#63+#74 is the most likely decline** and the plan says so in advance: if the ordering flip does not pay at M3, it is reverted and `Line`'s identity token survives with #74 named as its sole remaining reader (the Global Constraint on static mutable state records that outcome).

**Tests (named, binding).**
- `crates/fr-router/tests/tightener.rs::acid_trap_avoidance_is_off_by_default` and `…::with_the_setting_on_a_trace_springs_over_the_obstacle` — #182, on the new acid-trap fixture, with the hand-computed expectation reviewed here. The existing three regime tables are re-pointed, not deleted.
- `crates/fr-router/tests/control.rs::the_smd_relaxation_is_a_setting` — #172; asserts `minNormalViaCost` is **4000** with the relaxation off and **400** with it on, and that `attachSmdAllowed` and the `ViaMask` **agree** either way.
- `crates/fr-router/tests/failure_log.rs::an_item_that_fails_fifty_times_is_skipped_when_enabled` — #235; and `…::the_give_up_policy_is_disabled_by_default`.
- `crates/fr-dsn/tests/network.rs::via_at_smd_reaches_the_net_class_use_via_rule` — #104.
- `crates/fr-router/tests/tightener.rs::the_start_corner_contact_is_chosen_by_geometry` — #210; the `{496, 495, 494}` case, asserting **496** (the nearest) rather than 494 (the id-order last), with the geometric rule named in the test.
- `crates/fr-board/tests/board.rs::the_item_walk_is_ascending` and `crates/fr-geometry/tests/line.rs::polyline_change_compares_lines_by_value` — #44/#63/#74, **only if the flip is kept**; if reverted, these tests are reverted with it.

**Evidence / acceptance.** G1 green after each commit. **#182, #172, #235 and #104 move B/C on the stems they touch**; **#210 moves R/B on 45° stems**; **#44+#63+#74 moves EVERYTHING** (survey's Δrefs is literally "everything") and is regenerated in its own commit so the diff is attributable. G2 per row, separately.

**MILESTONE M3** — the G3 block **exactly as written**, with `--run-id plan9-m3`, compared `--runs java-278fe14,plan9-m3` into `benchmark/reports/plan9-m3-vs-java-278fe14.{json,md}` (`git add -f`), `remote-run.sh`'s output to `benchmark/baselines/plan9-m3.log`, and the workbench binary's sha pasted against `meta.json`'s. It is the **whole branch against the frozen `java-278fe14` run** — **`java-current` is not re-run here or anywhere in this plan** (ruling BN; an earlier draft's "against `java-current`" read as a live jar re-run and was wrong). **Acceptance:** `overall.verdict` **`better`**, zero hard losses, the D3-small numbers at or above M1's, and the **corpus-median `cpu_s` ratio against M2** inside BO's 20 % band or escalated with a keep/rework ruling. M3 is also where each of the six rows is confirmed or reverted. Read into Task 25.

**Steps:**
- [ ] Commit 1: the acid-trap fixture + #182 as a setting, default off.
- [ ] Commit 2: #172 as a setting, default on (today's behaviour); its A/B with the via column.
- [ ] Commit 3: #235 as a setting, default disabled; its A/B.
- [ ] Commit 4: #104.
- [ ] Commit 5: #210 — geometry, not id order.
- [ ] Commit 6: #44 + #63 + #74 as **one** ordering-flip commit with its own whole-corpus regeneration.
- [ ] Commit 7: **M3** — the workbench build + sha check, `remote-run.sh … --run-id plan9-m3`, the frozen-view compare; `git add -f benchmark/reports/plan9-m3-vs-java-278fe14.{json,md}`; the M2→M3 cpu ratio; each of the six confirmed or reverted.
- [ ] Register: each row → `fixed: T18` or back to `pinned` with "measured and declined at M3" and the number.

**Commit message:** `feat(router): the measured policy switches — #182, #172, #235, #104, #210 and the #44+#63+#74 ordering flip; M3 bench, each row confirmed or declined`

---

### Task 19: DRC report accuracy — survey group 19

**The DRC report is the artefact a KiCad user actually reads**, and the benchmark's "self-report vs DRC disagreement on 45–63 % of boards" is this task measured from the outside. **Survey §10.1 constraint 14: #195 lands before the acceptance table may use the length breakdown** — from this task on, G2's forbidden-input list shrinks.

**Files:** `crates/fr-drc/src/{report/mod.rs,report/json.rs,unconnected.rs,violations.rs}`; `crates/fr-board/src/items/mod.rs` (`smallest_clearance` — the field is **deleted**); `crates/fr-board/src/board/mod.rs` (`get_all_clearance_violations`); `crates/fr-router/src/score/statistics.rs` (the length breakdown, the bounding box — **Task 13 edited this file's fanout block for #194; re-read it, do not rely on the line numbers this plan quotes, ruling BP6**); `crates/fr-dsn/src/writer/{ses_writer.rs,rules_writer.rs}`; `crates/fr-board/src/datastructures/delaunay.rs` (`validate` — **Task 13 rewrote this file's in-circle predicate and bounding triangle; `validate()` is untouched by it and Task 24 deletes the private PRNG afterwards, ruling BP6**); `crates/fr-router/src/optimize/item_route_result.rs`; `crates/freerouting/src/commands/drc.rs`; `crates/fr-drc/tests/{report.rs,unconnected.rs}`; `crates/fr-board/tests/{clearance_violations.rs,snapshot.rs}`; `tests/reference/drc-*/**` (all 8).

**Interfaces consumed:** `fr_drc::{DesignRulesChecker, DrcReportOptions}` (Plan 5), `fr_core::PARITY_VERSION` (Plan 8).
**Interfaces produced:** `--fail-on-violations` and `--unit <mm|um|inch|mil>` on `drc`; `ClearanceViolation::smallest_clearance(&[…])` as a fold; `DrcJsonFlavor::KiCad` as the only **writer**.

**The fix list (11 rows).**

| # | mechanism | fix sketch |
|---|---|---|
| **#152** | `isHole` calls **every `Pin` a hole**, surface-mount pads included — and **its own comment admits it**. A pad with no drill has no hole to keep clear of; KiCad reports the overlap as `clearance` and freerouting as `holeClearance`, and the description's first three words change with it. **BBD Mars-64 splits 64 `holeClearance` against 12 `clearance` on this predicate.** | `item instanceof Via \|\| (item instanceof Pin pin && pin.getPadstack().fromLayer() != pin.getPadstack().toLayer())`. **Changes counts per `type`, not the total.** **This is the "per-pin vs per-pair" family the benchmark sees.** |
| **#146** | The dangling-trace dedup guards each candidate on `firstItem` **alone**, so a trace that is a net entry's `secondItem` — or merely a member of its `allItems` — is **reported twice**. The **via** phase has **no guard at all**. 17 of 112 rows reach it. The port already fixed #144's hash-dependence, so it emits a stable **112** where the jar says 113–115 across hash modes — **a permanent XDIFF on `drc-natural-tone-preamp`**. | Build the `allItems` set **once before the phase** and test membership of it (which also removes the O(n²) rescan), **or** drop the guard and let the two phases be independent as the via phase already is. **Not a number to tune toward any particular jar run.** **Closing this retires the tree's one permanent XDIFF** — the one artefact that exists only because the jar disagrees with itself. |
| **#153** | `Item.smallestClearance` is initialised to `-1.0` **once, at construction**, and only ever lowered — **a monotone minimum over the whole life of the item, never reset** — while `getAllClearanceViolations` runs once per report *and* once per `BoardStatistics` built with violations. **So by the end of a run every item reports the worst clearance any earlier call saw, even if routing improved it.** | **Drop the field** and let `ClearanceViolation.smallestClearance` **fold over the returned list** — it exists only because the GUI wanted a number the compute had thrown away. **Also kills the repeated whole-board recompute.** |
| **#271** | `initializeDrc` returns `true` **unconditionally**, so `-drc` exits **0** whatever it finds: a missing `.rules` warns and carries on, a missing session warns and carries on, a failed quality score warns, and **the violation count never reaches the exit code at all** — a board with 107 violations exits exactly like a clean one. | **Add `--fail-on-violations`** rather than silently redefining exit 0 (**recommendation 5**) — every existing CI script reads today's code. Optionally a distinct code for "ran, found violations"; default: **exit 1 under the flag, 0 without it**, documented in the help and in the hand-off's divergence table. |
| **#272** | The DRC report's quality score uses a **different settings merge** from the router's: the DRC path is `0,10,20,55,60` with **no `RulesFileSettings` at 40**, no between-merges pass, no second merge and no post-merge `.rules` re-apply. So `-de b.dsn -dr weights.rules -drc r.json` reports violations computed **with** those clearances and a `qualityScore` computed with weights **that file never influenced**. | **Use the job's own merged `routerSettings`** — the DRC path already has one and throws it away. **One line**, and it **discharges `crates/fr-settings/src/resolve.rs`'s obligation marker.** |
| **#151** | The report's coordinate unit is hard-coded `"mm"` at its only CLI call site, so **four of the five `convertCoordinate` branches are dead — including the fallback, the only arm that honours a board's declared `(unit …)`**. Every Issue575 fixture declares `(unit um)` and is silently rescaled by 1/1000, and `%.4f` means a `"um"` report would print four decimals of a micrometre — **the resolution of the text output changes with a parameter nothing can set.** | Give `drc` a **unit flag** (or read `board.communication.unit`) and route it through `Unit::from_string`. CLI wiring plus one flag; **the port has already ported all five arms.** Default stays `mm` so no committed D golden moves for this row alone. |
| **#154** | The report's first key advertises KiCad's **snake_case** schema and HEAD writes camelCase (`coordinateUnits`, `kicadVersion`, `unconnectedItems`, `holeClearance`) — **the document does not validate against the schema it names**, and 2.3.0 did. | **Make `DrcJsonFlavor::KiCad` the only flavour written** (recommendation 6, adopted; ruling W already made it the CLI default). With parity gone, `FreeroutingHead`'s reason to exist — "the parity choice the crate's tests pin" — is gone too: **keep the reader for one release, delete the writer arm.** |
| **#195 + #196** | The horizontal/vertical/angled breakdown walks `polyline.lines` and measures each **infinite line's two defining points**, including the two bounding lines that carry **no segment** — so **the three lengths do not sum to `totalLength`** (121 606.75 vs 130 610.65). `bounding_box.width`/`height` are handed the board's **lower-left corner** and read negative. | Walk `corner(i)`/`corner(i+1)`, which `totalSegmentCount` four lines above **already counts**; pass `ur - ll`. **From this commit on, G2 may use the length breakdown and the bounding box** (BL2's forbidden-input list shrinks and the change is announced in the commit message). |
| **#110** | `SesWriter.writeConductionArea` writes the boundary with `writeScopeInt` and each **hole** with plain `writeScope`, so a conduction area with holes emits `Double.toString` coordinates **inside an otherwise all-integer SES file**. A session file's grid is integral by construction. | Give `Shape.writeHoleScope` an **`int` variant**. Fixture `p8t13-conduction-area.dsn` already exists (Plan 8 Task 13), so the roadmap's "needs a new fixture" is discharged. |
| **#111** | `RulesWriter.writeRules` writes the design name **raw** where `DsnWriter` quotes it, so a name holding a space produces a `.rules` header **the reader's own `NAME` lexeme cannot read back as one token.** | `identifierType.write(designName, file)`. **Changes the first line of every `.rules` file.** |
| **#81, #212, #201** | Three diagnostics that lie: `PlanarDelaunayTriangulation.validate()` accumulates only on its leaf branch, so "check the consistency of the triangles" **answers `true` unconditionally** and logs "check passed ok"; `ItemRouteResult.improvementPercentage` divides two `int`s so the via term truncates (a re-route halving the via count scores like one removing **every** via — and the optimizer's own recomputation **has** the `(float)` cast, so **the field is wrong and the number actually used is right**); `getHash`'s javadoc says it hashes the **trace** state where it hashes the whole item graph — **the wrong comment is what justified this port's original trace-and-via-only hash**, under which every trace-free board hashed alike. | `result &= child.validate();`, the `(float)` cast, and **reword the three comments** (the third is a comment fix whose *consequence* — the port's hash — was already corrected; the row records why). |

**Also here, with no work owed:** **#144/#145** are already answered better by the port (`BTreeMap`/`BTreeSet` with a lowest-id representative; always `.`) — their register rows gain a note that **#144 is why `-XX:hashCode=2` was mandatory on `p8t3`, and retiring the jar comparison retires that requirement.** **#149/#150** (the hard-coded `focusNets = {98, 99}` debug block and the array-instead-of-count log line) are **NOT-A-FIX**: not ported, nothing owed. **#155** is a settings row and lands in **Task 21**, after #115.

**Tests (named, binding).**
- `crates/fr-drc/tests/report.rs::an_smd_pad_is_not_a_hole` — replaces `smd_pins_are_classified_as_holes`; asserts the BBD Mars-64 **64/12 split** moves to the corrected split and that **the total is unchanged**.
- `crates/fr-drc/tests/unconnected.rs::a_dangling_trace_is_reported_once` (four existing tests re-pointed) and `…::the_via_phase_dedups_too`; `…::natural_tone_preamp_reports_a_stable_count` — the XDIFF's retirement.
- `crates/fr-board/tests/clearance_violations.rs::smallest_clearance_is_a_fold_over_the_current_list` — #153; asserts a second call after an improvement reports the **improved** number.
- `crates/freerouting/tests/cli_e2e.rs::fail_on_violations_exits_one` and `…::drc_without_the_flag_still_exits_zero` — #271.
- `crates/freerouting/tests/cli_e2e.rs::the_quality_score_uses_the_rules_file_weights` — #272; the `obligation:` marker at `resolve.rs` is struck in the same commit.
- `crates/freerouting/tests/cli_e2e.rs::the_drc_unit_flag_reaches_the_report` — #151, all four previously-dead arms.
- `crates/fr-drc/tests/report.rs::only_the_kicad_flavour_is_written` — #154; the `FreeroutingHead` **reader** test stays.
- `crates/fr-router/tests/score.rs::the_three_lengths_sum_to_the_total` and `…::the_bounding_box_holds_extents_not_a_corner` — #195/#196, with the **121 606.75 / 130 610.65** pair as the fail-before evidence.
- `crates/fr-dsn/tests/parity_ses.rs::a_conduction_area_hole_is_written_as_integers` — #110, on `p8t13-conduction-area.dsn`.
- `crates/fr-dsn/tests/rules_round_trip.rs::a_design_name_with_a_space_round_trips` — #111.
- `crates/fr-board/tests/delaunay.rs::validate_fails_on_an_inconsistent_inner_node` — replaces `validate_is_vacuous_on_an_inner_node`; `crates/fr-router/tests/optimizer.rs::improvement_percentage_does_not_truncate_the_via_term` — replaces its pinning twin; `crates/fr-board/tests/snapshot.rs`'s hash comment corrected.

**Evidence / acceptance.** G1 green. **All 8 D stems regenerate** — #152 (per-type counts), #146 (row counts), #153 (the clearance numbers), #154 (every key's spelling) and #195/#196 (the statistics block) each move the document. `goldens moved:` carries **one line per fix per stem family**. **`p8t3`'s permanent XDIFF is deleted** and its deletion is the headline of #146's commit. `p7t7` re-golden for #195/#196. **G2's forbidden-input list shrinks after #195+#196**, which is a **gate-version bump to `g3`** (ruling BP8): the same commit re-cuts Task 12's affected baseline columns under the new gate, commits both, and the commit message names the bump. G2 rule otherwise unchanged.

**Steps:**
- [ ] Commit 1: #152. Commit 2: #146 (+ the XDIFF row deleted). Commit 3: #153.
- [ ] Commit 4: #271 (`--fail-on-violations`). Commit 5: #272 (+ the obligation struck). Commit 6: #151 (`--unit`).
- [ ] Commit 7: #154 (writer arm deleted, reader kept). Commit 8: #195 + #196 (+ the G2 announcement).
- [ ] Commit 9: #110. Commit 10: #111. Commit 11: #81, #212, #201.
- [ ] Commit 12: regenerate all 8 D stems + the C stems' `drc.json`; `goldens moved:`.
- [ ] Register: eleven rows → `fixed: T19`; #144/#145 annotated; #149/#150 marked `candidate` with "not ported, nothing owed".

**Commit message:** `fix(drc): the report tells the truth — #152 holes, #146 the dedup (permanent XDIFF retired), #153, #271, #272, #151, #154, #195+#196, #110, #111, #81/#212/#201`

---

### Task 20: manifest and statistics truth — survey group 20

**One object, one task** (survey §10.2): every row here is a field of the result manifest that lies, and fixing them one at a time would re-cut the same 13 C-stems repeatedly.

**Files:** `crates/fr-core/src/{manifest.rs,stats_from_bytes.rs,stats_json.rs}`; `crates/fr-router/src/score/statistics.rs` (**#291's drill count — Tasks 13 and 19 edited other blocks of this same file first; the "`batch.ses` byte-identical across this whole task" assertion covers this edit too, ruling BP6**); `crates/freerouting/src/commands/route.rs`; `crates/fr-core/tests/{manifest.rs,stats.rs}`; `tests/reference/cli-*/manifest.json` (13); `scripts/differential/run.sh` (`p8t2probe` retires here).

**Interfaces consumed:** `fr_router::score::BoardStatistics` (Plan 7/8), `fr_core::RoutingResultManifest` (Plan 8 Task 4).
**Interfaces produced:** `BoardStatistics::host: Option<String>`; `phases.{fanout,optimizer,autorouter}` all written; `RouterJobResourceUsage` reduced or dropped.

**The fix list (7 rows).**

| # | mechanism | fix sketch |
|---|---|---|
| **#247** | `countOccurrences` is a **substring** count with no token boundary, so `(layer` counts `(layer_rule`, `(net` counts `(network` **and** `(net_class`, `(via` counts `(via_rule`, `(class` counts `(class_class`. An eleven-clause DSN with one of each reports **2, 3, 2, 2**. The SES branch additionally **drops a real layer whose `(path ` clause ends the file.** | **Match on a token boundary** and read `words[0]` whenever the chunk is non-empty. **Changes every count the result manifest reports.** |
| **#248 + #249** (**#250 already-fixed**) | The DSN `host` scrape **can never succeed on a real Specctra file**: `searchLimit` is the first `)` after `(parser`, which in every real DSN closes `(string_quote ")`; the keywords are HEAD's own camelCase `(hostCad`, which no exporter writes; and the value slice **hard-codes exactly one character after the keyword**. All **fourteen** corpus DSN rows report `host` **null**. **#249:** the fallback that would have written `"Freerouting," + VERSION` is **unreachable**, because a Java string concatenation of two nulls is the six characters `null,null`. **#250** (`(parser (hostCad))` slicing backwards out of a constructor with no `catch`) is **already totalized** in the port. | **Balance the parentheses**; accept `(host_cad`/`(host_version` beside the camelCase; **skip whitespace after the keyword**; **null-test the two fields before concatenating.** **A fixed scrape puts a real `host` in every manifest, which no committed reference expects — a deliberate, wanted re-baseline.** |
| **#254** | `phases.fanout` and `phases.optimizer` are allocated and **never written** — a manifest whose whole point is per-stage duration reports two of three stages as `{}` for ever — and `phases.autorouter.duration_seconds` holds **the whole job's wall clock** (board load, fanout, router, optimizer, SES write). *A harness reading it to compare routers is reading the process's wall clock.* | **Write all three stages** (both already time themselves) and **rename the autorouter's field to `total_seconds` or move it to the top level.** **Land with #267/#230** (Task 9 split the fields; this fills the keys). |
| **#255 + #256** | `sha256Hex` swallows **every** failure into `null` and Gson omits the key, so "the file was deleted", "the path is a directory", "the path is unreadable" and "there was no input" produce **indistinguishable JSON** — for a field whose whole point is fixture identity. And `resource_usage.io_read`/`io_written` are **never assigned anywhere in the tree** yet are `float` primitives, so every manifest carries `0.0, 0.0` — **two fields that state a measured quantity and are always the same lie.** | **Let the hash error out, or write an explicit `null`.** **Delete `io_read`/`io_written`**, and either measure the other three or **drop the object** — the port cannot measure them without a forbidden dependency, so **dropping is the honest answer** and it **deletes `normalize_manifest`'s special case.** |
| **#251** | Port divergence: `BoardStatistics.host` is a `String`, so **the port cannot tell Java's `null` from `""`.** One input reaches it, and it is an XDIFF row on both sides. Ruling **BD** already measured the blast radius: **1 declaration, 3 writes, 6 reads**, ~4 lines of the `p8t2` transcript. | `Option<String>`. **The scan-ruling R3/R15 additive-only constraint on `fr-router` was a Plan 8 constraint and does not survive into Plan 9** — this is the row that proves it. **Land with #248.** |
| **#291** | `items.drill_item_count` is **always 0**: `instanceof Pin` is tested **before** `instanceof DrillItem` and **`Pin` *is* a `DrillItem`**, as is `Via` — the only two concrete subclasses. A manifest reader asking "how many drilled items" is told **`0` for a board of 400 pins and 90 vias.** | **Decide which meaning the field has**: test `DrillItem` first **and rename `pin_count`/`via_count`**, or delete the counter. **Do not just reorder** — that makes the other two unreachable instead. Default: **test `DrillItem` first and keep all three, with `pin_count`/`via_count` computed from the concrete types**, so all three are non-zero and mean what they say. |
| **#236** (optional) | `BoardScoreBreakdown.of` reads `stats.traces.totalLength` — **raw board units** — while its javadoc and the live `calculateScore` use `totalLengthMm`, so on any board whose DSN resolution is not 1 a breakdown "explanation" **does not add up to the score it explains.** Both it and `ScoringWeightComparison` are **dead in `main/`** and unported (ruling AS). | The roadmap wanted these ported as the A/B report renderer. **That argument is weaker now**: `benchmark/`'s referee already has a corpus-scale verdict function (whose noise floor, at the one seed the milestones run, is zero — the hard metrics are exact because the router is deterministic). **Port them only if a per-stem explanation is wanted, and fix the unit first.** **Default: do not port; record the decision and the unit bug in the register row, and keep the ruling-AS roster line.** |

**Tests (named, binding).**
- `crates/fr-core/tests/stats.rs::layer_rule_is_not_counted_as_a_layer` (and three siblings for `(net`, `(via`, `(class`) — #247; the eleven-clause DSN's **2, 3, 2, 2** becomes **1, 1, 1, 1**.
- `crates/fr-core/tests/stats.rs::a_ses_layer_whose_path_ends_the_file_is_counted` — #247's SES branch.
- `crates/fr-core/tests/stats.rs::a_real_specctra_file_yields_a_host` — #248; asserts a **non-null** host on all fourteen corpus DSN rows, and `…::snake_case_host_keywords_are_accepted`.
- `crates/fr-core/tests/stats.rs::the_host_fallback_is_reachable` — #249.
- `crates/fr-core/tests/manifest.rs::all_three_phases_are_written` and `…::the_autorouter_duration_is_not_the_whole_job` — #254.
- `crates/fr-core/tests/manifest.rs::an_unreadable_input_is_distinguishable_from_a_missing_one` — #255; `…::the_resource_usage_object_is_gone` — #256, plus `normalize_manifest`'s special case deleted.
- `crates/fr-core/tests/stats.rs::host_is_optional` — #251.
- `crates/fr-core/tests/manifest.rs::drill_item_count_counts_pins_and_vias` — #291; the literal for a 400-pin/90-via board is **490**.

**Evidence / acceptance.** G1 green. **All 13 C-stem manifests regenerate**, once. `goldens moved:` carries one line per field: *"#247 — layer/net/via/class counts fall to their true values; #248 — `host` becomes non-null on 14/14 DSN rows; #254 — `phases.fanout`/`phases.optimizer` are populated and `autorouter.duration_seconds` is renamed; #256 — `resource_usage` is removed; #291 — `drill_item_count` becomes non-zero."* **`p8t2probe` RETIRES** (BL7): final MATCH count recorded (289 lines today), the pair deleted, the Rust half's table kept as `crates/fr-core/tests/stats.rs` literals **re-derived for the fixed behaviour, not copied from the jar**. `p8t2 e2e` re-golden. G2 unaffected (no routed board changes) — and that is asserted: **`batch.ses` must be byte-identical across this whole task**.

**Steps:**
- [ ] Commit 1: #247. Commit 2: #248 + #249 + #251 (one commit — the survey pairs #251 with #248). Commit 3: #254. Commit 4: #255 + #256. Commit 5: #291.
- [ ] Commit 6: #236's decision recorded (default: not ported), the register row's unit bug written down.
- [ ] Commit 7: retire `p8t2probe`; regenerate the 13 manifests; `goldens moved:`; assert `batch.ses` byte-identical.
- [ ] Register: seven rows → `fixed: T20` (#236 → `candidate`, decision recorded); #250 stays `totalized`.

**Commit message:** `fix(core): the manifest stops lying — #247 token-boundary counts, #248+#249+#251 a real host, #254 three phases, #255+#256, #291 drill items`

---

### Task 21: the settings merge engine — survey group 21

**Ordering inside the task is fixed** (survey §10.1 constraint 10): **#128 → #126 + #127**, and **#115 → #155** (a primitive `false` is inexpressible until #115 lands).

**Files:** `crates/fr-settings/src/{merge.rs,field_path.rs,router_settings.rs,dsn_file_settings.rs,resolve.rs,scoring.rs}`; `crates/fr-drc/src/lib.rs` (#155's `DrcSettings`, re-introduced or deleted); `crates/fr-core/src/manifest.rs` (`from_job`'s guard); `crates/fr-settings/tests/{copy_fields.rs,field_path.rs,router_settings.rs}`; the **per-layer-trace-width fixture** from Task 11; `scripts/differential/run.sh` (`p4t1` retires here).

**Interfaces consumed:** the per-layer-width fixture (Task 11), `fr_settings::SettingsMerger` (Plan 4).
**Interfaces produced:** `MaxPasses::{Unlimited, Limited}`; a `ScoringSettings` whose weights are **non-optional**; a property-path resolver that **refuses** `private` and `static` fields.

**The fix list (10 rows).**

| # | mechanism | fix sketch |
|---|---|---|
| **#128** | `DsnFileSettings` seeds the layer count via `setLayerCount`, which also **replaces both per-layer trace-cost arrays with fresh all-`1.0` arrays** — and `copyFields`' primitive-array rule is **first writer wins**, so from priority 20 onward those `1.0`s **cannot be replaced**. **A `.rules` file at 40, an environment variable at 55 and a `--router.*` flag at 60 all carry per-layer trace costs that are silently dropped.** Per-layer trace costs are what steer **direction preference**. | **Seed only `layers`** — inline the array-sizing half, or give `setLayerCount` a variant that does not touch `scoring`. **Fixing #126 does not fix this** (here the arrays start `null`). Uses the **per-layer-width/cost fixture** built at Task 11. |
| **#126 + #127** | `boardSpecificTraceCostsApplied` is `private transient` and `copyFields` skips non-`public` fields, so **a merged `RouterSettings` inherits the source's tuned cost arrays with the flag back at `null`** — and `applyBoardSpecificOptimizations` **re-initialises exactly the costs the merge just carried across**. **#126:** `setLayerCount` wipes every per-layer cost even when the count is **unchanged**, but only clears the flag when it **reallocates**. | Make the flag **public** (it is already `transient`, so no JSON key appears) or copy it explicitly; **move the cost resets inside the reallocation branch.** **Land after #128.** |
| **#119** | A property path through an array field sizes a `null` array at **the value's token count**, not the board's layer count, and writes `min(len, size)` elements **with no error** — and `applyRouterSettingsForLoadedBoard` then calls `setLayerCount(boardLayerCount)` whenever the counts disagree, resetting `routable`/`preferredDirectionHorizontal`/`bendCost` on **every** element. **So a `--router.layers.*` whose arity does not equal the board's layer count is silently discarded wholesale.** | **Size from the board and reject a disagreeing token count.** **Turns silence into an error message, which is the point.** |
| **#115 + #116** | `copyFields` can **never merge a `false` or `0` primitive**: `shouldCopy` requires `!sourceValue.equals(getDefaultValue(field))` and `getDefaultValue` returns the *type's* default — so a source that explicitly wants `include_warnings = false` **cannot express it**. **#116:** a `Set`/`List` field falls to the generic rule and **recurses into `HashSet`'s own private fields** — the merge is a **silent no-op**, so `DebugSettings.filterByNet` loaded from JSON never survives. | **Box the primitives so `null` means unset** — the nullable-wrapper convention the whole merger is already built on — and **special-case `Collection`/`Map`.** **Precondition for #155.** |
| **#155** | `DesignRulesChecker.drcSettings` is stored and **never read**; `includeWarnings`/`includeErrors` filter nothing and `enabled` is `transient` so it does not survive the Gson round trip the class exists for. | **Either read the two flags in `generateReport`** (filter by `severity`, already `"error"`/`"warning"`) **or delete the class.** Default: **read them** — the flags are a real user need and #115 has just made `false` expressible. **Strictly after #115.** |
| **#121 + #118** | `getFieldByNameOrSerializedName` does not filter by modifier and `setFieldValue` calls `setAccessible(true)`, so **`private` and `static final` fields are both reachable from a property path** — `--router.board_specific_trace_costs_applied=true` **defeats the guard #127 depends on**, and `min_bend_cost` resolves to a `public static final` constant and throws. **#118:** the path splitter's class is `[.:\-]`, so **a hyphen is a separator** and `optimizer-max_passes` silently means `optimizer.maxPasses`. | **Skip `static` and non-`public` fields as `copyFields` already does** — *a property path is external input and must not reach a private invariant flag* — and **drop `-` from the class.** |
| **#140** | `RouterSettings.validate` is **not idempotent**: it maps `maxPasses == 0` ("no limit") to `Integer.MAX_VALUE`, and `MAX_VALUE > 9999` so the **next** call maps it to `9999`. The headless path merges **twice**, each merge ending in an unconditional `validate()` — so `--router.max_passes=0` is a **200 000-fold** difference in the routing budget decided by **how many merges ran**. | **A separate `unlimited` flag** (`MaxPasses::Unlimited`) and **stop calling `validate` once per merge.** Plan 8's constraint "`validate()` is never called a third time" becomes "validate is idempotent", which is stronger. |
| **#142** | The scheduler's `.rules` file is **parsed twice against two different layer structures**: at priority 40 the structure is discovered **from the file itself**, so a file naming `F.Cu`/`B.Cu` describes a *two*-layer stack; after the merge the identical bytes are re-parsed against `board.layerStructure`, so on a four-layer board that same `B.Cu` rule lands on layer **3**. **Both results are applied — 13 disagreeing rows of 84.** | **Parse once, against the board when there is one**, or make the discovered structure explicit in the result. Default: **parse once against the board.** Same family as #112 (Task 4). |
| **#124, #125, #114, #139** | Four one-liners: `validate` and `normalizeMaxThreads` **disagree about `maxThreads == 0`**, so a `0` that reached the field through the merge survives and the pool would be size zero; `validate` **unboxes two `Integer`s that are `null`** on any `RouterSettings` that did not pass through `DefaultSettings`, throwing out of `merge()`; `clone` **omits `resultJsonPath`**; `getRunFanout`/`isFanoutEnabled` carry the **same javadoc and opposite defaults**. | The four one-liners the register names. **#124 stays a precondition for any future parallelism.** **#114's fix makes `java_clone` equal to `#[derive(Clone)]`** — Task 24 collects it. |
| **#258** | A `RoutingJob` still holding its field initialiser's `new RouterSettings()` makes `fromJob` **NPE** as soon as the job has a board, because `new ScoringSettings()` leaves **every weight `null`** and the guard tests the *object*, not the weights. Not reachable from the headless CLI; **live for any API job whose JSON omits the weights.** | **Make the weights non-optional in the type** — the port can express what Java cannot. A silent default would change which board the pass loop keeps, which is why Plan 7 chose the panic; **a required field changes nothing at runtime and removes the panic.** |

**Tests (named, binding).**
- `crates/fr-settings/tests/copy_fields.rs::a_rules_file_per_layer_trace_cost_survives_the_merge` — #128, on the per-layer-width fixture; the same test at priorities 40, 55 and 60.
- `crates/fr-settings/tests/copy_fields.rs::a_merged_settings_keeps_its_costs_and_its_flag` and `…::an_unchanged_layer_count_does_not_wipe_the_costs` — #126 + #127.
- `crates/fr-settings/tests/field_path.rs::a_layer_array_of_the_wrong_arity_is_refused` — #119.
- `crates/fr-settings/tests/copy_fields.rs::an_explicit_false_merges` and `…::a_set_field_merges` — #115 + #116.
- `crates/fr-drc/tests/report.rs::include_warnings_false_filters_the_warnings` — #155.
- `crates/fr-settings/tests/field_path.rs::a_private_field_is_not_reachable_from_a_property_path`, `…::a_static_final_field_is_not_reachable`, `…::a_hyphen_is_not_a_separator` — #121 + #118.
- `crates/fr-settings/tests/router_settings.rs::validate_is_idempotent` and `…::max_passes_zero_means_unlimited_after_any_number_of_merges` — #140.
- `crates/fr-settings/tests/rules.rs::a_rules_file_is_parsed_once_against_the_board` — #142; asserts the **13 disagreeing rows of 84** collapse to 0.
- `crates/fr-settings/tests/router_settings.rs::{max_threads_zero_is_refused, validate_does_not_unbox_a_null, clone_carries_the_result_json_path, the_two_fanout_accessors_agree}` — #124/#125/#114/#139.
- `crates/fr-core/tests/manifest.rs::from_job_needs_no_weight_guard` — #258; the `expect` with Java's message is deleted by name.

**Evidence / acceptance.** G1 green. Most rows are **U-only**; **#128 and #142 move B/C on boards with a rules file**, and #128's per-layer trace costs **steer direction preference**, so its per-stem effect is named. `goldens moved:` names #128 and #142. **`p4t1`'s 64-case settings matrix RETIRES** (BL7): final MATCH count recorded (5 728 lines today), the pair deleted, and **the 64 cases are re-derived as `fr-settings` unit tests against the FIXED semantics** — not copied from the jar, which is now wrong about ten of them. G2: incompletes not up; the rules-carrying stems' scores reported.

**Steps:**
- [ ] Commit 1: #128. Commit 2: #126 + #127. Commit 3: #119.
- [ ] Commit 4: #115 + #116. Commit 5: #155 (strictly after 4).
- [ ] Commit 6: #121 + #118. Commit 7: #140. Commit 8: #142.
- [ ] Commit 9: #124, #125, #114, #139. Commit 10: #258.
- [ ] Commit 11: retire `p4t1`; re-derive the 64 cases as unit tests; regenerate B/C where #128/#142 moved them; `goldens moved:`.
- [ ] Register: ten rows → `fixed: T21`; Plan 8's constraint 10 note re-worded to "validate is idempotent".

**Commit message:** `fix(settings): the merge engine becomes predictable — #128, #126+#127, #119, #115+#116, #155, #121+#118, #140, #142, #124/#125/#114/#139, #258`

---

### Task 22: CLI predictability — survey group 22

**The largest single behaviour change in T4, and it breaks command lines that work today.** **Recommendation 4 is adopted with its ramp:** fix the **native** subcommand form immediately (exact matching, hard failure), and give the **legacy** flag form **one release of "matched by prefix, warned as deprecated"** before it becomes exact — `-decoy a.dsn` setting the design input is a bug, but somebody's script may be relying on a prefix today and the port has no telemetry to know. **The deprecation warning names the flag it matched and the exact spelling to use.**

**Files:** `crates/freerouting/src/{cli.rs,legacy.rs,logging.rs,main.rs}`; `crates/fr-settings/src/sources/cli.rs` (`convert_value`, the empty-value rule); `crates/fr-core/src/{file_details.rs,job.rs}` (`set_filename`, `change_file_extension`); `crates/fr-core/src/load.rs` (`load_board_if_needed`); `crates/fr-settings/src/resolve.rs` (`resolve_scheduler_rules_path`); `crates/fr-dsn/src/lexer/{scanner.rs,number.rs}` (**#84/#85: the skip/stop sets and the number grammar. `scanner.rs` was already edited at Task 4 (#86 deleted the 16 MiB input ceiling) — different function, eighteen tasks earlier, already regenerated; re-read the file before editing and do not re-introduce the ceiling — ruling BP6, named at both ends**); `crates/freerouting/tests/cli_e2e.rs`; `docs/cli-legacy-flags.md`; `scripts/differential/run.sh`, `scripts/differential/sweep-p8t5.sh` (**both retire here**), `scripts/differential/matrix/p8t5-argv.tsv`.

**Interfaces consumed:** `fr_settings::CliSettings` (Plan 4), `fr_core::BoardFileDetails` (Plan 8 Task 1).
**Interfaces produced:** one flag table shared by both parsers; a `-v`/`--verbose` on the native form; `java_path`'s POSIX-only pin removed from `set_filename`.

**The fix list (6 rows).**

| # | mechanism | fix sketch |
|---|---|---|
| **#259 + #262** | `applyCommandLineArguments` **warns but never fails**, and **every short flag except `-l` is matched with `startsWith`**: **`-decoy a.dsn` sets the design input** and **`-drcx r.json` enters DRC mode**; a missing value is **silently ignored and `i` is not advanced**; `-mp -5` is inexpressible; `-mp abc` produces two log lines. **Nothing on this path can make the process exit non-zero.** **#262:** Java parses argv **five times** and the passes disagree — `-dlx` disables file logging in one and not another, `-ll trace -ll error` configures log4j at `TRACE` while the settings read `ERROR`; and `--mcp_server.stdio=true` redirects stdout **without starting a server**. Port: `legacy.rs::resolve_slots` reproduces it bug-for-bug (ruling AR); all **84** `p8t5` rows + the 87-row sweep pin it. | **Match flags exactly on both paths and fail on an unknown flag or a missing value.** Then #262's `-mpx` hazard disappears and **the two remaining parsers share one table**. **Ramp (recommendation 4):** the legacy form keeps prefix matching for one release **with a deprecation warning naming the matched flag and the exact spelling**; the native form is exact **now**. The ramp's expiry is recorded in `docs/cli-legacy-flags.md` and in Task 25's report. |
| **#120 + #136** | `convertValue`'s boolean arm is `Boolean.parseBoolean`, i.e. `"true".equalsIgnoreCase(s)` — **every other string is `false`, silently.** So **`--router.enabled=yes` disables the router**, **`--router.vias_allowed=on` forbids vias**, `--router.strict_drc=ture` reads as an **explicit `false`**, and `" true "` is false too. **#136:** an **empty** value counts as an explicit choice from the property name alone, disarming `-de`/`-do` batch forcing and then reading as `false` — **a trailing `=` turns routing off.** | Accept `true`/`false`/`0`/`1`, **case-insensitive and trimmed**, and **throw otherwise**; treat an empty value as "not provided" **or** reject it (default: **reject**, naming the flag). **Pure predictability, no routing change on a correct command line.** #120's fix is also what makes `java_parse_bool` dead (Task 24). |
| **#131-#135 + #132** | The five legacy optimizer flags `-oit`, `-us`, `-is`, `-hr`, `-inc` and `-drc`'s router switch write a bridge **nothing reads** — in Java they have **no effect at all**. Compounded: the bridge matches by **prefix** while the live path matches **exactly** (so a typo reaches the dead half), `-mp` is `Integer.decode` on one and `parseInt` on the other, `-oit -5` never reaches its clamp, and one `-mt` token writes two fields with two clamps that never touch. | **Wire the five onto the live path, match exactly on both, one parser.** **Makes the port strictly more capable than the jar** — a recorded product decision (ruling AQ hands it here), **not drift**. Ruling AQ's "the dead legacy flags stay dead" is **superseded by this task** and Plan 8 hand-off §3 row 7 is struck (Task 25 carries the edit). **`java_integer_decode` dies with it** (Task 24). Land with #259/#262. |
| **#263 + #274 + #269** | Three "diagnosed in the wrong place" rows. A **bare `-drc`** sets two dead booleans and then dies in the CLI branch with "Both an input file and an output file must be specified" — **blaming a `-do` the user did not need**. `-de prev.ses -drc` is refused by the **loader, three steps after the mistake**, with a message naming a format the user never typed. And **`-dr` with a non-existent path silently disables** the adjacent `<design>.rules` discovery, so a typo loses the rules file sitting next to the design **with no warning**. | **Bare `-drc` means "check with the default report path"** (or refuses **naming the missing path**); **refuse a session file at the argument**, since `-de` knows the extension and `setInput` knows the bytes; **make the `-dr` branch fall through when the file does not exist, and warn either way.** **Also close #273's asymmetry**: `initializeDrc` reads only `initialRulesFile` where the router path also probes for an adjacent `<design>.rules`, so the same directory gives the two modes different rules. (**#273's ORDER stays** — survey §9.1: the rules-first board is the correct one, measured both ways.) |
| **#246 + #242** | `BoardFileDetails.setFilename` runs **Windows-only string surgery unconditionally**: the branch tests `File.separator` against the caller's **raw** string while every value below comes from the **absolute** path; `replaceAll("[/\\\\]+$", "")` turns the parent of `/board.dsn` — **the root** — into the empty string, so a file at the root reports a **relative** absolute path; and `replaceAll("\\\\.$", "")` is the regex "a backslash then **any** character", so a POSIX directory named `dir\x` becomes `dir`. **#242:** `changeFileExtension` **NPEs on every bare filename** and returns two different *kinds* of path depending on whether the extension already matched. | **Delete the two Windows rewrites**; branch on `getNameCount() > 1`; **keep the root as the parent it is**; compute the parent lazily inside the two branches that use it and **return the reconstructed absolute path from all three.** **Note the asymmetry is load-bearing today** — `setInputFromFile` relies on it for an `.frb`/`.json` input's derived default output — so **the derived-output rule must be written explicitly in the same change.** This is also the module whose **POSIX-only pin** is what Windows support would have to unpick, and removing the rewrites is the first half of that. |
| **#260, #87, #84, #85, #137, #130, #138** | Seven documentation and lexer predictability rows: the help documents **11 of 23** accepted forms and **there is no `-v`**; `-ll garbage` **silently means `INFO`**; `Keyword.GENERATED_BY_FREEROUTING` is **unreachable from the scanner**, so `dsnFileGeneratedByHost` stays `true` **even for a DSN freerouting itself wrote**; the lexer's skip/stop sets contain **8 (backspace) and not 9 (tab)** although the comments say "spaces, tabs"; `nextToken`'s DFA and `nextDouble` implement **two different number grammars in one file** (`1e5` is 100000.0 as a token and 1.0 through `nextDouble`); `-de board.brd` **drops the file** rather than failing at the argument; `SesFileSettings`' javadoc claims to read SES files and `loadSettings` **opens nothing**; and `SettingsSource`'s javadoc puts the GUI at **50** where the constant is **65**. | **Add the missing lexer rule** (then **re-check every consumer of `dsnFileGeneratedByHost`**); **use `{9, 32}`**; **parse both numbers with the DFA's grammar**; **fail at the argument with the extension named**; **delete the SES rung**; fix the one wrong number; **document all 24 forms and add `-v`** (recommendation 12: CLI first; the MCP gets it only where the tool already exposes the neighbouring option). **The lexer pair (#84/#85) changes what a DSN token *is*** and is the only one here with corpus reach — **`java_number_format_parse` dies with #85** (Task 24). |

**Tests (named, binding).**
- `crates/freerouting/tests/cli_e2e.rs::an_unknown_native_flag_fails` and `…::a_missing_value_fails` — #259.
- `crates/freerouting/tests/cli_e2e.rs::a_prefix_matched_legacy_flag_warns_and_names_the_spelling` — the ramp; `-decoy a.dsn` still works **and** warns.
- `crates/freerouting/tests/cli_e2e.rs::argv_is_parsed_once_and_the_log_level_agrees` — #262.
- `crates/fr-settings/tests/cli.rs::{yes_is_not_false, on_is_not_false, ture_is_an_error, a_trailing_equals_is_refused}` — #120 + #136.
- `crates/freerouting/tests/cli_e2e.rs::the_five_legacy_optimizer_flags_reach_the_live_path` — #131-#135, one assertion per flag.
- `crates/freerouting/tests/cli_e2e.rs::{a_bare_drc_checks_with_the_default_report_path, a_session_file_at_de_is_refused_at_the_argument, a_missing_dr_path_falls_through_to_the_adjacent_rules}` — #263/#274/#269; and `…::both_modes_find_the_same_adjacent_rules` — #273's asymmetry.
- `crates/fr-core/tests/file_details.rs::{a_file_at_the_root_reports_an_absolute_path, a_posix_directory_with_a_backslash_survives, a_bare_filename_changes_extension}` — #246 + #242; plus `…::the_derived_default_output_rule_is_explicit`.
- `crates/fr-dsn/tests/lexer.rs::{a_tab_is_whitespace_and_a_backspace_is_not, one_number_grammar, a_dsn_freerouting_wrote_is_recognised}` — #84/#85/#87; and every consumer of `dsn_file_generated_by_host` re-checked by a named grep pasted into the commit.
- `crates/freerouting/tests/cli_e2e.rs::{ll_garbage_is_refused, brd_at_de_fails_at_the_argument, the_help_documents_every_form, dash_v_exists}` — #260/#130/#137.

**Evidence / acceptance.** G1 green. **`p8t5` and `sweep-p8t5.sh` RETIRE** (BL7) — they exist to pin the bug-for-bug behaviour this task deletes. Final counts recorded (`p8t5` MATCH, 2 124 lines; the sweep, 87 rows / 87 MATCH), the pair and the matrix deleted, and **the argv matrix is re-derived as `cli_e2e.rs` cases against the FIXED behaviour** — 87 rows in, a smaller, honest set out, and the mapping is written into `docs/cli-legacy-flags.md`. **`p8t1probe` RETIRES** with #246+#242 (its `setFilename`/`changeFileExtension` tables are exactly what this task changes; final count 162 lines). **G moves via #84/#85** — the lexer pair changes what a DSN token is, so the 6 `fixtures.txt` rows and the `fr-dsn` round-trip goldens re-cut, and `sweep-p3t15.sh` is re-run over the 106-fixture corpus with any newly-disagreeing fixture named. `goldens moved:` names #84/#85. G2 unaffected except where #84/#85 changes a parse — asserted stem by stem.

**Steps:**
- [ ] Commit 1: #259 + #262 + #131-#135 — one flag table, exact on native, prefix-with-warning on legacy, the five flags wired.
- [ ] Commit 2: #120 + #136.
- [ ] Commit 3: #263 + #274 + #269 (+ #273's asymmetry).
- [ ] Commit 4: #246 + #242 (+ the explicit derived-output rule).
- [ ] Commit 5: #84 + #85 (+ the `dsn_file_generated_by_host` consumer re-check).
- [ ] Commit 6: #260, #87, #137, #130, #138 — the doc/lexer tail, including `-v` and all 24 forms.
- [ ] Commit 7: retire `p8t5` + sweep + `p8t1probe`; re-derive the argv matrix as `cli_e2e` cases; regenerate G; `sweep-p3t15.sh`; `goldens moved:`.
- [ ] Register: six rows → `fixed: T22`; ruling AQ's rows (#131, #143) annotated "superseded by Plan 9 Task 22" for #131; **#143 (`-mt` dead) stays dead** — no threading policy is introduced.

**Commit message:** `fix(cli): predictability — #259+#262 one exact parser, #120+#136 booleans, #131-#135 the five flags wired, #263+#274+#269, #246+#242, #84+#85 and the doc tail`

---

### Task 23: DJ1 — the shims std already has — survey group 23

**Zero FIX rows.** Cleanup, scheduled here by BL5 because none of it changes a routed board. **The task's own acceptance is that no golden moves.** If one does, the shim was load-bearing, the row becomes **KEEP**, and the finding is recorded.

**Files:** `crates/fr-geometry/src/java_compat.rs` and its ~110 call sites across all eight crates; `crates/fr-board/src/rules/*` (the name-matching sites, done **under a directed test**); `crates/fr-geometry/src/rational_point.rs` (`java_big_integer_hash_code`, after the read audit); every crate README's shim paragraph.

**Interfaces consumed:** none. **Interfaces produced:** the REPLACE-STD set is deleted; the KEEP set is **renamed** (behaviour untouched).

**The REPLACE-STD list, verbatim from survey §8.1 (~110 occurrences).**

| shim | sites | replacement | note |
|---|---|---|---|
| `java_double_compare` / `java_float_compare` | 9 / 13 | `f64::total_cmp` / `f32::total_cmp` | **Exactly** `Double.compare`'s total order (NaN last, `-0.0 < 0.0`). Zero behaviour change, zero churn. **#147's NaN guard (Task 13) is already spelled with it.** |
| `java_trim` / `java_is_whitespace` / `java_is_blank` | 34 / 8 / 24 | `str::trim`, `char::is_whitespace`, `str::trim().is_empty()` | Java's `trim` strips `<= U+0020`; Rust's strips Unicode whitespace, NBSP included. **Reachable only through a value containing NBSP or an exotic space, which no corpus input has.** #120's fix (Task 22) wants ordinary `trim()` anyway. **Verify on the `p8t2probe` table before it retires** — it retired at Task 20, so the verification is against the Task-20 commit's recorded transcript. |
| `java_to_upper` / `java_to_lower` (+ `_unit` variants) | 24 / 13 | `to_uppercase` / `to_lowercase`; ASCII sites → `eq_ignore_ascii_case` | Agree with Java on everything the corpus contains (including `ß`→`SS`). **CAUTION: the clearance-class and net-name comparisons in `fr-board/src/rules` are name matching on USER DATA and a non-ASCII net name exists in the tests (`GND_é中`, quirk #290) — convert those with a directed test, not in bulk.** |
| `java_string_cmp` / `java_string_compare` | 9 / 6 | `str::cmp` | Java orders by **UTF-16 code unit**, Rust by Unicode scalar value; they differ **only for supplementary-plane characters** (emoji, rare CJK), which no board name in the corpus has. Both sites are DSN library/network ordering, so **the churn is bounded to G and is one-time**. |
| `java_big_integer_hash_code` | 6 | `#[derive(Hash)]` | **Verify FIRST that nothing observable reads it** (it should be a `HashMap` key only). **If any ordering or output depends on it, it becomes KEEP and the *dependency* is the bug** — recorded as a new register row in that case. |

**The rename-only pass (rides along, mechanical, no behaviour change).** The KEEP set keeps its semantics and loses the `java_` prefix, because ~700 of the ~900 occurrences are wire contracts, geometry rules or NaN-propagation semantics this plan **depends on**: `java_double_to_string` → `dsn_double` (170 sites — this **is** the DSN/SES number format); `java_round_to_int` → `round_half_up_to_i32` (113, and the `long`→`int` truncation stays **documented**); `java_min`/`java_max`/`_f32`/`java_math_max`/`java_abs_f32` → `min_nan_propagating`/`max_nan_propagating` (260 — **Rust's `f64::min` silently swallows a NaN, exactly the failure mode #9 and #170 are about; swapping them would hide the defects this plan fixed**); `java_round`/`java_round_half_up`/`java_rint` (111 — two genuinely different rules at genuinely different sites, not conflated); `JavaNumberFormatter` (19, Gson's number rendering — a wire contract); `java_format_fixed` → `format_fixed_half_up` (40 — the DRC report's `%.4f` is read by KiCad users and by the schema); `java_double_stream_sum` → `kahan_sum` (6 — *more* accurate than a naive fold, and it feeds `normalized_score`, a threshold comparison in two loops); the name/enum/class-name contracts (~40 — **JSON values and log payloads consumers match on; changing one string is a wire break for no gain**); `java_parse_i32`/`_f64`/`_f32`/`java_parse_*_vec` (~39 — their overflow and format rules **are** the documented CLI contract).

**Tests (named, binding).**
- `crates/fr-board/tests/rules.rs::a_non_ascii_net_name_still_matches_its_clearance_class` — the `GND_é中` case (quirk #290), the **one** directed test the caution above demands, written **before** the `fr-board/src/rules` conversion.
- `crates/fr-geometry/tests/rational_point.rs::the_big_integer_hash_has_no_observable_reader` — the read audit, expressed as a test that greps the workspace and asserts the only uses are `HashMap`/`HashSet` keys.
- Every existing test is unchanged; **that is the point.** No new behavioural test is written in this task.

**Evidence / acceptance.** G1 green **with NO regeneration**. That is the acceptance, stated as a gate: `git status --porcelain tests/reference/` is **empty** after the task. If any golden moves, the task **stops**, the moving shim is reverted to KEEP, a register row records why, and the rest of the list proceeds. `sweep-p3t15.sh` re-run (the `java_string_cmp` sites are DSN ordering). `grep -rc 'fn java_\|struct Java' crates/*/src` recorded before and after in the commit message.

**Steps:**
- [ ] Commit 1: the `GND_é中` directed test (failing is not expected; it is the guard).
- [ ] Commit 2: `java_double_compare`/`java_float_compare` → `total_cmp`.
- [ ] Commit 3: `java_trim`/`java_is_blank`/`java_is_whitespace`.
- [ ] Commit 4: `java_to_upper`/`java_to_lower`, with `fr-board/src/rules` done last and separately verified.
- [ ] Commit 5: `java_string_cmp`/`java_string_compare`; `sweep-p3t15.sh`.
- [ ] Commit 6: the `java_big_integer_hash_code` read audit and, if clean, `#[derive(Hash)]`.
- [ ] Commit 7: the rename-only pass over the KEEP set; **mechanical, no behaviour change, gate 1 green**.
- [ ] Register: survey §9.1's "§8's KEEP shims" row updated with the new names; no row changes status.

**Commit message:** `refactor: DJ1 — the shims std already has (~110 sites) and a rename-only pass over the KEEP set; no golden moves`

---

### Task 24: DJ2 — the shims a fix deleted — survey group 24

**Zero FIX rows. This task writes nothing new; it is the bill the fixes already paid, collected.** **Strictly after Tasks 7, 8, 13, 21 and 22** (survey §10.2: DJ2 is downstream of ten fixes and must not start before all ten have merged). The ten are: **#82** (T13), **#160/#161** (T8), **#171** (T8), **#280** (T7), **#282** (T7), **#85** (T22), **#114** (T21), **#118** (T21), **#120** (T22), **#247** (T20).

**Files:** `crates/fr-router/src/collections/java_tree_set.rs` (deleted); `crates/fr-board/src/datastructures/delaunay.rs` (the private `JavaRandom` copy at `:81` and its `shuffle`); `crates/fr-dsn/src/kicad/{java_hash.rs,npe.rs}` (deleted); `crates/fr-settings/src/java_compat.rs` (`java_clone`, `java_number_format_parse`, `java_parse_bool`, `java_integer_decode`, `java_split*`); the ten one-off reproductions across four crates; `crates/fr-geometry/src/random.rs` (`JavaRandom` → `SeededLcg`); `crates/fr-core/tests/shims.rs` (**new** — the keep-list gate below lives here); every affected README.

**Interfaces consumed:** the ten landed fixes. **Interfaces produced:** `SeededLcg` (BL3); `BTreeSet` where `JavaTreeSet` was; `#[derive(Clone)]` where `java_clone` was.

**The FOLD-INTO-FIX list, verbatim from survey §8.1 (~150 occurrences), each with the fix that killed it.**

| shim | sites | killed by | what goes with it |
|---|---|---|---|
| `JavaTreeSet` (+ `JavaTreeSetIter`) | 75 / 15 files | **#160+#161 and #171+#170** (Task 8) | Its whole reason to exist is the **non-total comparators**. Both are now total, so `JavaTreeSet` → **`BTreeSet` and no element is lost**. The two remaining users — `pipeline/fanout.rs` (whose comparator, after #219, can only tie on a duplicate `pinIndex`, i.e. never) and `board_ext/routing_board_ext.rs` — **convert for free**. **Deleting `JavaTreeSet` IS the acceptance test for #160/#161 + #171**, and the deletion commit says so. |
| `delaunay.rs`'s private `JavaRandom` + its `shuffle` | 1 file | **#82** (Task 13) | Existed only to reproduce Java's insertion order in the triangulation #82 rewrote. With an exact in-circle predicate the edge set is **shuffle-independent** (Task 13's `the_edge_set_is_independent_of_insertion_order` proves it), so the copy **goes entirely**, or becomes one call into `fr-geometry`'s `SeededLcg`. **Do not duplicate a PRNG across two crates any longer.** |
| `JavaStringSet` / `JavaStringMap` / `java_hash_iteration_order` / `java_string_hash` | 7 / 8 / 7 / 9 | **#280** (Task 7) | All four existed for KiCad auto-registered net numbering **alone**. **~30 sites and one of the most intricate mechanisms in the port, removed by a one-word behaviour change** — plus the **treeification `debug_assert!`** the Plan 8 hand-off parked at §6 task 9 (already deleted at Task 7; its absence is asserted here). |
| `JavaNpe` | 9 | **#282/#285/#287** (Task 7) | Existed to reproduce the crash *messages*. DTO-boundary validation replaced them with real diagnostics, and the `package_pin_names` / `net_name_is_null` side tables go with it. |
| `java_clone` | 23 / 7 files | **#114** (Task 21) | Once `resultJsonPath`'s omission is fixed the semantic is "copy every field", which is `#[derive(Clone)]`. **Keep the explicit version only if the merger genuinely needs a *partial* clone — check at fix time and record the answer.** |
| `java_number_format_parse` | 7 | **#85** (Task 22) | This **is** #85 — two number grammars in one file. #85's fix parses both with the DFA's grammar, which deletes the shim. |
| `java_parse_bool` | — | **#120** (Task 22) | `"yes"` reading as `false` **is** #120. |
| `java_integer_decode` | 4 | **#131-#135** (Task 22) | The two-parser split **is** #131-#135. |
| `java_split` / `java_split_literal` / `java_split_underscore` | 19 / 3 / 1 | **#118** (Task 21) and **#247** (Task 20) | `field_path.rs`'s use is #118's `[.:\-]` class, whose fix removes `-`; `stats_from_bytes.rs`'s is #247's SES scrape, whose fix rewrites the chunk walk. **What is left after both is one `split` with an explicit "drop trailing empties" — which Rust spells `split(..).filter(..)`, in place.** |
| the ten one-off reproductions: `java_shift_loop_hangs`, `java_nets_get`, `java_drill_item_tile_shape_count`, `java_parent_is_null`, `java_double_to_int`, `java_int`, `java_reverse`, `java_fixed_state`, `java_simple_uppercase_exception`, `java_static_constants_are_not_settable_fields` | 1-5 each | #241 (already-fixed), #282 (T7), #286 (T7), #242 (T22), #121 (T21) | Each is tied to a named row. **`java_shift_loop_hangs` in particular is a public predicate whose only consumer is the `p8t1probe` driver Task 22 retired**, so it is unreferenced by the time this task runs. |

**Also here: BL3's rename.** `fr-geometry`'s public `JavaRandom` (59 sites, 11 files) becomes **`SeededLcg`** — **implementation unchanged**, only the claim that it matches the JVM dropped. `PolygonShape.splitToConvex` picks its concavity-scan start from it, so shape division — and therefore every routed board — is reproducible **only if the generator is**; the bit-specified LCG satisfies BL3's cross-version and cross-platform requirement already. **Quirk #30's per-call construction stays** (it is already better than Java's `static` field). **No new dependency is added.**

**Also here: `fr_geometry::Line`'s identity token.** #218 (Task 14) removed one of its two readers. If Task 18 kept the #74 ordering flip, the second reader is gone too and **the token and the static counter are deleted**, taking the port's last static-mutable exception with them. If Task 18 declined #74, **the token stays**, this task says so at the site, and the Global Constraint's sentence is amended to record the outcome. **Either way the answer is written down.**

**Tests (named, binding).**
- `crates/fr-router/tests/collections.rs` is **deleted** with `JavaTreeSet`; its element-drop assertions were already replaced by Task 8's total-order tests, and the deletion commit names them.
- `crates/fr-geometry/tests/random.rs::seeded_lcg_is_value_stable` — a fixed seed's first 16 outputs as literals, so a future refactor cannot silently move a golden (BL3's whole point).
- `crates/fr-dsn/tests/kicad_nets.rs` and `crates/fr-board/tests/delaunay.rs` are unchanged and must stay green — **they are the proof that the deleted emulation was dead.**
- `crates/fr-core/tests/shims.rs::no_java_prefixed_shim_survives_outside_the_keep_list` — a workspace grep test whose allow-list **is** survey §8.1's KEEP column (under its new names). This is the gate that keeps the deletion honest.

**Evidence / acceptance.** G1 green **with NO regeneration** — same gate as Task 23: `git status --porcelain tests/reference/` empty. **`grep -rc 'fn java_\|struct Java' crates/*/src` recorded before and after**; the expected drop is **~150 occurrences** on top of Task 23's ~110, leaving the ~700 KEEP occurrences under their new names. If a golden moves, the deletion was not a consequence of a landed fix and it is reverted with the finding recorded.

**Steps:**
- [ ] Commit 1: verify all ten upstream fixes have merged (a named grep per fix, pasted).
- [ ] Commit 2: `JavaTreeSet` → `BTreeSet` (75 sites) — **the acceptance test for #160/#161 + #171**.
- [ ] Commit 3: `delaunay.rs`'s private PRNG.
- [ ] Commit 4: the four `HashMap`-emulation types (~30 sites).
- [ ] Commit 5: `JavaNpe` and the two side tables.
- [ ] Commit 6: `java_clone`, `java_number_format_parse`, `java_parse_bool`, `java_integer_decode`, the `java_split` family.
- [ ] Commit 7: the ten one-off reproductions.
- [ ] Commit 8: `JavaRandom` → `SeededLcg` + the value-stability test; the `Line` identity-token decision recorded either way.
- [ ] Commit 9: `no_java_prefixed_shim_survives_outside_the_keep_list`; the before/after counts.
- [ ] Register: no status changes; survey §8.1's FOLD-INTO-FIX rows annotated "collected at T24".

**Commit message:** `refactor: DJ2 — the shims ten fixes deleted (~150 sites), JavaRandom -> SeededLcg; no golden moves`

---

### Task 25: the completion report, the whole-branch review, the fix wave, the merge and the v1.1.0 tag

**Files:** `docs/plan-9-handoff.md` (**new** — the post-parity completion report); `docs/plan-9-prep/review-findings.md` (**new** — the whole-branch review's findings); `docs/java-quirks.md` (the final status sweep); `docs/plan-8-handoff.md` (§3's struck divergence rows, §7's struck limitations); `scripts/differential/run.sh` (**`p8t6` retires here** — ruling BP11); `benchmark/reports/java-regressions-2026-09.md` (the closing status section) and `benchmark/reports/plan9-m{1,2,3}-vs-java-278fe14.{json,md}` (read, and force-added if a milestone's own commit did not); every crate README's Plan 9 paragraph; `docs/superpowers/plans/2026-09-03-plan-9-post-parity.md` (the amendment log, filled).

**No fix rows.** This task produces the evidence that the plan is done, gets the branch reviewed, absorbs the review, merges and tags.

**1. The whole-branch review (Fable).** A single review of `main..plan-9-post-parity` — every commit, not a sample — against five questions, each of which has a mechanical check behind it so the review cannot be a vibe:
   1. **Does every FIX row have a directed test that failed before it?** Check: the register's `fixed: T<n>` rows against the commits' `directed:` gate lines. Any row without one is a finding.
   2. **Does every moved golden have a named reason?** Check: `git log --format=%B main..HEAD | grep -c 'goldens moved:'` against the number of commits that touched `tests/reference/`.
   3. **Is any `// Java bug:` marker deleted?** Check: `grep -rn '// Java bug:' crates` ≥ 165 (A11), and every `fixed:` register row has a `// fixed: T<n> (#id)` marker naming it — which `crates/fr-core/tests/register.rs` runs, so the check is the test rather than a grep.
   4. **Did any fix add recovery Java lacks?** Check: the `catch_unwind` boundary count is still **seven**, and `grep -rn 'catch_unwind' crates/*/src` names the same seven sites Plans 6–8 documented.
   5. **Is any constraint quietly broken?** Check: `grep -rn 'unsafe' crates/*/src` empty; `grep -rn 'thread::spawn\|rayon::' crates/fr-*/src` empty; `git diff main..HEAD -- '*/Cargo.toml'` shows **no new dependency** (or exactly the one BL3 authorised, with its amendment).

**2. The fix wave.** Every review finding is fixed on the branch, in its own commit, with its own G1. A finding that cannot be fixed is a **recorded, closed decision with its evidence** — the Plan 8 precedent — never a deferral to a Plan 10.

**3. The completion report, `docs/plan-9-handoff.md`.** Eight sections, mirroring Plan 8's shape so a reader who knows one knows the other:
   1. **What was fixed** — all 121 FIX rows in one table: register id, tier, task, the one-line mechanism, the direction of the change, and the golden family it moved. This is the document a future reader diffs the jar against.
   2. **What was measured** — M1, M2 and M3's verdicts side by side (run-ids `plan9-m1/m2/m3`, all rs-only on the workbench against the frozen `java-278fe14` view, their compare reports force-added under `benchmark/reports/`), with the **`v1.0.0-rs` run of record** as the pre-plan position and the report's ablation numbers beside them; the **corpus-median `cpu_s` ratio chain** `v1.0.0-rs → M1 → M2 → M3` (BO/BP4) with every escalation and its ruling; plus the G2 table's 29 stems at M3, at gate-version `g3`, and the rolling `benchmark/baselines/` artefacts that produced them.
   3. **What was measured and declined** — every Task 18 row that M3 reverted, with its number. **A declined row is a result, not a gap.**
   4. **The harness, after** — which drivers retired and their final MATCH counts (`p6t2`, `p2t13`'s modes, `p4t1`, `p8t1probe`, `p8t2probe`, `p8t5` + sweep, and `p8t6`'s status), which five converted to port-golden comparison, the `--against-jar` escape hatch, and **the two JVM flags that disappeared with them** (`-XX:hashCode=2` for #144, the locale pair for #145).
   5. **The frozen baseline** — what `tests/reference-frozen/java-head-2026-09/` is (a **sibling** of `tests/reference/`, read-only since Task 1), why it exists, and the rule that **nothing reads it**: the jar numbers derived from it live at `benchmark/baselines/quality-baseline-java-head.tsv` and the `grep -rn "reference-frozen" crates/ scripts/ benchmark/` check returns only prose.
   6. **The register, closed** — every row's final status; the count of `fixed:` rows; the count of `pinned` rows that survive **with the reason each survives** (the survey's KEEP set, the investigations that stayed open, #193's recommendation).
   7. **Deliberate divergences from the jar** — Plan 8 §3's 22 rows **updated**: rows **7** (dead legacy flags — superseded by Task 22) and **10** (`-do out.json` — fixed at Task 3) struck with their replacement; the new Plan 9 divergences added (`--fail-on-violations`, `--unit`, `-v`, exact flag matching with the legacy ramp and **its expiry date**, the KiCad-only DRC flavour, insertion-ordered KiCad nets, the settings the policy rows introduced and their defaults).
   8. **Known limitations and upstream candidates** — Plan 8 §7 updated: **#162 struck** (fixed at Task 8), **#113 survives** (the `(string_quote .)` regex divergence — no regex engine, still true); plus **recommendation 8's standing upstream list**: R1 (#293), R2 (#294), #105 (the jar hangs), #162 (OOM), #241/#244 (two CLI hangs), #95 (settings silently dropped), #27, #39, #144/#145 — each with the Plan 9 commit that contains the patch a PR would carry.

**4. The merge.** Fast-forward or a merge commit into `main`, with the whole-branch summary in the message: commits, tests before and after, the three milestone verdicts, the golden families re-cut, and the shim count before and after.

**5. The tag proposal: `v1.1.0`.** Proposed, not created unilaterally — the message drafted here and the tag cut on the user's word. Rationale written into the report: **the port's behaviour is no longer that of freerouting 2.3.1-SNAPSHOT**, so a version that reads as "the port of 2.3.x" would be false. `1.1.0` says: the same program, one minor step on from the parity release, with a documented, measured behaviour change — and `PARITY_VERSION` **stays what it is** (it names the jar each format surface was pinned against, and that is still true of the formats).

**Tests (named, binding).**
- `crates/fr-core/tests/register.rs::a_fixed_row_has_a_fixed_marker` — armed at Task 0, now **non-vacuous**: it must pass over ~121 rows.
- `crates/fr-core/tests/register.rs::every_java_bug_marker_has_a_register_row` — still green, count ≥ 165.
- `crates/fr-core/tests/shims.rs::no_java_prefixed_shim_survives_outside_the_keep_list` — Task 24's gate, still green.
- The **full** suite, the five converted drivers, and `FR_SLOW_PARITY=1 cargo test --release`, all recorded in the merge message.

**Evidence / acceptance.** The five review checks pass with their outputs pasted. The report's eight sections are complete. `main` carries the branch. The tag is drafted and offered.

**Steps:**
- [ ] Commit 1: the whole-branch Fable review, findings recorded in `docs/plan-9-prep/review-findings.md`.
- [ ] Commits 2..n: the fix wave, one finding per commit.
- [ ] Commit n+1: `docs/plan-9-handoff.md`, all eight sections.
- [ ] Commit n+2: the register's final status sweep; `docs/plan-8-handoff.md`'s struck rows; the READMEs; the benchmark report's closing section; this plan's amendment log filled.
- [ ] Merge to `main` with the summary message.
- [ ] Propose `v1.1.0` with its drafted message; **do not create the tag without the user's word.**

**Commit message (the report commit):** `docs(plan9): the post-parity completion report — 121 fixes, three milestones, the harness after, the register closed`

---

## Dispatch order and sizing

**Twenty-six tasks, 0–25.** The order is the survey's own (§10.3's suggested sequence), with BL4's sequential-writer rule and BL5's placement of the cleanup tasks:

```
0 → 1 → 2 → 3 → 4 → 5 → 6 → 7 → 17 → 8 → 9 → 10 → 11 → 13 → 14 → 15 → 16 → 12 → 19 → 20 → 21 → 22 → 23 → 24 → 18 → 25
```

**Why the order is what it is.** Task 1 before every behavioural fix (the freeze is unrecoverable and the clock must leave the router before a golden is cut). Task 2 next (it moves the baseline every later A/B is taken against). Tasks 3–7 are the T1 rows and touch different crates, so their **internal** commits may be reordered — the tasks themselves do not overlap. **Task 17 runs before Task 8** because its answer may subsume several of Task 8's rows and it changes no behaviour, so it is free to run early. Task 12 sits after Task 11 in the survey's sequence rather than beside it, because #26's `area()` (Task 11) and #28+#29's `smallestRadius` (Task 12) are the same class's holes and the second reads better after the first. Tasks 19–22 are the T3/T4 tail. **Tasks 23 and 24 land with that tail (BL5)** and before Task 18, which is last on purpose: every row in it is a deliberate change of policy or of an arbitrary order, and each needs the rest of the catalogue underneath it before its A/B means anything.

**Milestones:** **M1** at the end of Task 2 (`plan9-m1`), **M2** at the end of Task 16 (`plan9-m2`), **M3** at the end of Task 18 (`plan9-m3`). **Three whole-corpus runs in the plan and no fourth** — all three rs-only on the workbench against the frozen `java-278fe14` view (rulings BN/BP3). #231's follow-up is a **stem A/B at Task 16**, not a bench arm; a corpus number for it would be a controller-authorised fourth run and is not authorised here.

**Sizing and reviewer.**

| task | size | model | reviewer | why |
|---|---|---|---|---|
| 0 | small | sonnet | sonnet | tooling, no behaviour |
| **1** | large | **opus** | **opus** | the freeze is one-shot and unrecoverable; #234 re-cuts every golden |
| **2** | large | **opus** | **opus** | the plan's headline; M1's numbers are the whole case |
| 3 | medium | sonnet | opus | three fixes, one path, data loss |
| **4** | large | **opus** | **opus** | seven fixes, two API changes, a coordinate-scale fix with a fixture decision |
| 5 | medium | opus | opus | a non-termination proof and a deliberate reversal of a Plan 7 parity repair |
| 6 | medium | sonnet | opus | sixteen guards; mechanical, but #22 has blast radius |
| **7** | large | **opus** | **opus** | six fixes, a DTO redesign, re-numbers every KiCad net |
| **8** | largest | **opus** | **opus, twice** | nine fixes, the biggest R/B churn after Task 2, two rows that touch the same door sets |
| **9** | large | **opus** | **opus, twice** | #227 is the single largest quality change in the catalogue |
| 10 | large | opus | opus | #231 re-baselines nearly every stem |
| 11 | large | opus | opus | ten geometry fixes; #186 is the most visible geometry change in the register |
| 12 | medium | opus | opus | implementations with no oracle; every expectation is hand-computed |
| **13** | large | **opus** | **opus** | an exact predicate, a deliberate ratsnest re-baseline, `AIRLINE_BUDGETS` |
| 14 | small | sonnet | opus | two rows, but #51 feeds the neckdown chain |
| 15 | small | sonnet | sonnet | three fanout rows |
| **16** | medium | **opus** | **opus** | ruling BP18: the smallest fix load in the plan carries the heaviest **measurement** load — M2, the I2 column, the #231 stem A/B arm and the M1→M2 delta table |
| 17 | small | sonnet | opus | measurement only; the byte-identity gate is what needs the review |
| **18** | large | **opus** | **opus, twice** | six policy rows, each abandonable; M3 is here |
| **19** | large | **opus** | **opus** | eleven rows; all 8 D stems re-cut; the permanent XDIFF retires |
| 20 | medium | sonnet | opus | seven manifest fields; `batch.ses` must not move |
| **21** | large | **opus** | **opus** | ten settings rows with a hard internal order |
| **22** | large | **opus** | **opus** | breaks command lines that work today; the ramp must be exactly right |
| 23 | medium | sonnet | sonnet | ~110 mechanical sites; the gate is "no golden moves" |
| 24 | medium | sonnet | opus | ~150 deletions; the gate is "no golden moves" |
| **25** | large | **opus** | **opus** | the report, the review, the merge |

**Every review from Task 3 on re-runs `cargo test -p fr-router --test batch_parity` and `run.sh p6t1` on the committed tree, not on the implementer's word.** From Task 2 on, every review additionally reads the task's `goldens moved:` lines and spot-checks one moved golden by eye.

---

## Amendment log

One line per amendment made during execution: what the plan said, what the tree said, and the ruling. Task 0 opens it; every task appends. **An implementer who finds a signature, a file:line or a count different from what is written here stops and reports** — it is a further amendment, never a silent adaptation.

| # | task | what this plan said | what the tree said | ruling |
|---|---|---|---|---|
| A1 | plan-wide | the milestones ran a **local**, two-candidate `bench run` (`java-current` **and** `rs-main`) over a comma-joined list of three prose tier names at **three** seeds, with no `--run-id`, into a `benchmark/`-relative `out/M<n>/` directory, and compared with **no `--runs`** | that output directory does not exist and nothing writes to it (`run` writes `results/<run-id>/`, `compare` writes `reports/`); `corpus.py::select` takes **one** tier string and no tier of any of those three names exists in the manifest, so the argument selects zero boards; both runs of record used one seed on the workbench; and a compare without `--runs` silently takes "the latest run per candidate" rather than the frozen java view | **BN + BP3.** The G3 block is rewritten from the harness: `scripts/remote-run.sh --remote-dir freerouting-rs/benchmark --jobs 12 --poll-interval 300 -- --candidates rs-main --candidates-file candidates.rs.toml --tier pcbench --max-passes 10 --timeout 300 --threads 1 --run-id plan9-m<n>`, then `uv run bench compare --baseline java-current --against rs-main --runs java-278fe14,plan9-m<n> --tier pcbench`. `java-current` is never re-run. |
| A2 | 1 | freeze by recursively copying `tests/reference/` into a **child of itself**, make it read-only, then write the G2 baseline tsv **inside** it | a recursive copy of a directory into itself is not expressible, and the second step writes into a tree the first step just made read-only | **BP1.** Freeze to the **sibling** `tests/reference-frozen/java-head-2026-09/` via `rsync -a`, write the jar tsv to `benchmark/baselines/quality-baseline-java-head.tsv` **before** the `chmod`, and BL8 is amended so nothing — `quality-ab.sh` included — reads the frozen tree. |
| A3 | 1 | "`[candidates.rs-main]` … was uncommented at `c467758`" and a `bench run` baseline is recorded here | `c467758` changed four documentation files and no bench code; `rs-main` has been live since `ecc0abf`, the import | **BP2 + BP3.** Provenance corrected; Task 1 verifies `candidates.toml` itself rather than trusting a commit message; the baseline run is **deleted** — the pre-fix position is the existing `v1.0.0-rs` run of record. |
| A4 | plan-wide | timing is "reported, never optimised" | ruling BO puts speed into acceptance | **BP4.** `cpu_s` joins G2 against a rolling **port** baseline (`benchmark/baselines/stem-times.tsv`), with BO's thresholds, per-milestone corpus-median ratios, and Tasks 1/9/12/13 named in advance. The seeds-1 "2× seed stdev noise band" claim is deleted: at `seeds 1` the band is zero, and the real argument is that the router is deterministic. |
| A5 | 1, 2, 9 | `RouterBudget` lives in `crates/fr-core/src/ctx.rs`; one `autoroute_items(&mut self, …) -> (Vec<(ItemId, NetIndex)>, HandledItems)` | `RouterBudget` is declared at `crates/fr-router/src/pipeline/stop.rs:493` (`impl Default` at `:522`); the tree has **two** functions, `autoroute_items` `:702` and `autoroute_items_with_handled` `:714`, the latter being the p7t1 seam | **BP5.** `stop.rs` added to Task 1's Files; the two functions stay two and keep `&self` + `board: &Board`. |
| A6 | 8, 13/19/20, 4/22, 14 | cross-task shared files and hand-offs unnamed at one or both ends | five shared-writer pairs and one subsumption path with no mechanism | **BP6/BP7/BP15.** `scanner.rs` (T4×T22), `delaunay.rs` + `statistics.rs` (T13×T19×T20) named at both ends; Task 8 opens with a Task-17 subsumption table; per-task A/B artefacts are committed under `benchmark/baselines/ab/` and Task 14's joint neckdown table reads them. |
| A7 | plan-wide | G1: "zero diffs on every commit"; the evidence bar rejecting any fix that carried only the first four of its five items; `docs/java-quirks.md` in three Files lines out of 26 | BL4's cadence makes mid-task golden diffs unavoidable; a third of the plan's rows are U-only and have no A/B by construction; 23 tasks edit the register | **BP9/BP10/BP11.** G1 becomes per-commit (unaffected tests green + a stated `pending goldens:` line) and task-close (zero diffs); item (5) of the bar applies only where a fix moves a committed reference; a global Files note covers the register and the regenerated families, T6's ten guard sites are enumerated, and `register.rs` / `shims.rs` / `run.sh` / `benchmark/reports/` are added to T0 / T24 / T25. |
| A8 | 2, 16, 17, 21/20 | M1 has no exit on a miss; the #231 experiment "rides on M2's run"; Task 16 sized `sonnet`; the self-review said Task 21 lets Task 20 drop a guard | a missed threshold had no branch; an extra candidate arm is a second full corpus pass; Task 16 is the heaviest measurement task in the plan; the guard drop is Task 21's own and Task 20 runs first | **BP12/BP3/BP18/BP14.** M1 gains a controller-adjudicated fallback; the #231 arm is demoted to a Task-16 stem A/B; Task 16 dispatches at `opus`; the prose is corrected. |
| A9 | 0 | `gen-reference.sh` "grows a `--from-port` mode that drives `target/release/freerouting` with the same argv" | the **jar** lane of that generator does not drive the jar's CLI either — it compiles and runs `scripts/gen-reference/RefWriter.java`, because `-de/-do` cannot write DSN headlessly. The port reproduces that limitation, so `target/release/freerouting` has no DSN-write path and there is nothing for `--from-port` to drive | **Task 0 wrote the port's `RefWriter` twin**, `scripts/differential/rust/src/bin/refwriter.rs` — the same three arguments, the same two steps (`fr_dsn::read_board` then `dsn_writer::write` / `ses_writer::write`), which is the pair `crates/fr-dsn/tests/parity_dsn.rs` already asserts reproduces the jar's bytes. Proved: both lanes byte-identical to all seven committed `roundtrip.dsn` / `unrouted.ses`. One new harness file outside the task's Files line, recorded here per BP11. |
| A10 | 0 | `quality-ab.sh`'s quality columns "run with `RouterBudget::disabled()`" (ruling AI, G2 spec) | the CLI **hard-codes** `fr_core::RouterBudget::default()` at `crates/freerouting/src/commands/route.rs:340`, deliberately (`gen-cli-reference.sh`'s header argues it: `p8t1` compares two whole programs and the jar cannot switch its own javac-inlined budget off). There is no knob, and `quality-ab.sh` drives the CLI because its 29 stems are references *of* the CLI | **`FR_ROUTER_BUDGET`**, read at exactly one site (`route.rs::harness_budget`), values `default` (the unset case — Java's four literals, byte-for-byte what every user, test and committed golden gets) and `disabled`; an unrecognised value exits 2 rather than falling back silently. **Not** a settings-surface addition: it is not a flag, not mergeable, absent from the manifest snapshot, and `EnvironmentVariablesSource` cannot see it (that source reads `FREEROUTING__ROUTER__*` only). Measured: with and without it, `router-rpi-splitter`'s SES is identical. One file outside the task's Files line, recorded here per BP11. |
| A11 | 0, 25 | the `// Java bug:` census is "**≥ 165**", gated as `grep -c '// Java bug:' crates/*/src` | `crates/*/src` answers **148**. Survey §9.1's 165 is `grep -rn '// Java bug:' crates` — **every** file: **153** markers at `.rs` code sites (five of them in `crates/*/tests`, where a reproduced defect's only site is the assertion that pins it) plus **12** prose mentions in the seven crate `README.md`s | **`crates/fr-core/tests/register.rs` counts what §9.1 counted** and asserts both halves: total ≥ 165 and code ≥ 153. Task 25's gate should quote `crates`, not `crates/*/src`. The test excludes itself from both counts, because it discusses the marker in prose (`docs/java-quirks.md`'s own "the marker gate is two greps, not one" note, applied to itself). |
| A12 | 0 | `benchmark/baselines/stem-times.tsv` is "seeded from `results/v1.0.0-rs`'s **per-board** `cpu_s`" | the two corpora are **disjoint**. `v1.0.0-rs` is 605 `pcbench-*` boards; this file's 29 rows are the `tests/reference/` stems, and not one stem is a pcbench board — there is no per-stem number in that run to copy | **Seeded from Task 0's own median-of-3 measurement** of the same binary generation (the branch's base commit is what produced `v1.0.0-rs`), with the run of record written into the file's header as the **corpus-level** provenance every milestone's cpu ratio is taken against — read live from `benchmark/results/v1.0.0-rs` when present (605 cells, median 3.040 s, mean 25.199 s, total 15 245.2 s) and quoted when that gitignored directory is absent. |
| A13 | 0, 19 | G2's rule: "clearance violations — **0 on every stem, always**" | three of the eight DRC fixtures are *named for the violations they ship with* (`Issue575-drc_…_4_hole_clearance_violations.dsn` and two more), one routed stem is chosen as "the only board with pre-existing clearance violations" (`router-strict-drc-cnh`), and `hole_clearance` is a **different violation type** from `clearance` whose per-pin-vs-per-pair counting is itself survey §4.1's disagreement and Task 19's fix | **The rule is applied to the `clearance` type on routed stems.** A DRC stem routes nothing, so its count is a property of the board it was handed and is reported, not gated. `router-strict-drc-cnh` is a named `PRE_EXISTING` entry in the scorer and degrades to "must not rise", scored against the port baseline like every other column. `hole_clearance` gets its own reported column and no gate. Measured at T0: **zero** routed stem has a `clearance` violation, so the rule as stated is met today and the carve-outs are documentation rather than exemptions in use. |
| A14 | 0 | "prove `--jar` mode byte-unchanged … into a scratch dir and diffing against the committed tree" | the generators wrote only to `$ROOT/tests/reference`, so "into a scratch dir" was not expressible; and two committed fields are **not** reproducible by any lane — `drc.json`'s `date` and `cli-*/route.log`'s log4j2 timestamps — while `*.meta.txt` records the jar's own mtime, which moves when the clone is rebuilt | **`REFERENCE_OUT_ROOT`** added to all five generators: it redirects **output** to a scratch tree while the fixture tables and the `batch.ses` cross-check are still read from `tests/reference/`. The proof is over the reference **payloads** and is recorded in the Task 0 report with exactly what was compared. |
| A15 | 0 | `gen-cli-reference.sh`'s `batch.ses` cross-check is "a **failure**, not a note" | between Task 0 and Task 1's regeneration the two families sit in **different lanes** — a port-cut `route.ses` against a jar-cut `batch.ses` — and they differ by quirk #92's four head tokens and nothing else (3 656 B vs 3 654 B on `router-rpi-splitter`), which survey §9.1 keeps as a *decision* | The cross-check tries a plain `cmp` first and only then `parity::normalize_ses_head_tokens`' four rewrites, reporting which it used and why. Once both families are port-cut the normalisation is a no-op and the plain `cmp` answers first; a difference that is **not** those four tokens is still a FAILURE. |
| A16 | plan-wide | the G1 tier's second line is `scripts/differential/run.sh p8t1 ci` | `p8t1` takes **stem names**, `all`, or nothing; `select_stems` (`scripts/differential/rust/src/bin/p8t1.rs:86-100`) panics with *"no such stem in cli-fixtures.txt: ci"* on the word `ci`, because the CI lane **is** its no-argument default | **The G1 line is `scripts/differential/run.sh p8t1`.** It runs the same 10 rows the plan means (9 MATCH + 1 XDIFF — `invalid-input-java-hangs`, quirk #244). `run.sh`'s `p8t1` case already carries `default_args=()` for exactly this reason. |
| A17 | 0, and every task that lands a fix | the `// fixed: T<n>` marker "names the Plan 9 task and, in one clause, what the port does instead" | keyed on the task alone, **one** marker anywhere satisfies **every** row that task closes — a task fixing ten rows needs one comment — so `a_fixed_row_has_a_fixed_marker` could not check the "at every site" clause its own message and the register's rules header both state (Task 0 review, S3) | **The marker is `// fixed: T<n> (#id)`**, with the register row's id in parentheses. The test now requires a marker naming *that row*, and requires **one per site** wherever the site inventory is machine-readable — if `n` `// Java bug:` markers name `#id`, at least `n` `// fixed:` markers must name it back. Register rules header, marker-vocabulary row and test all updated together; proved by flipping #227 to `fixed: T9` and watching the gate fail, including against a marker naming a different id. Cheap now, expensive after 22 tasks have written markers in the loose form. |
| A18 | 0, 1 | `quality-ab.sh` carries "**two references per row** — the port baseline and the jar column" | the script **loaded** `benchmark/baselines/quality-baseline-java-head.tsv` and never referenced it again: no jar column in `COLUMNS`, no jar value emitted, and from Task 1 on the tsv header would have printed `# jar-baseline: <path>` with no qualifier — a harness affirming a reference it did not read (Task 0 review, **B1**, the one blocking finding) | Four **`jar_*` context columns** added to every row, rendered from that file and `-` until it exists; the header says which of the two worlds the tsv was written in and, when present, how many stems were read. No rule, no flag and no exit code reads them. **Task 1 must write the file in this script's own tsv shape** (`# gate-version:`, `# cols:`, `family`/`stem` keys) — stated in the script's header and in `benchmark/baselines/.gitkeep`. |
| A19 | 0 | (not stated) — the tsv's provenance line and its `cpu_s` floor | `port-sha` was a bare `git rev-parse HEAD` with no working-tree check, so the committed T0 tsv named the **base** commit, which does not contain `harness_budget()` — at that sha `FR_ROUTER_BUDGET=disabled` is ignored (Task 0 review, S1). And the cpu floor read only the *prior* run's spread with no absolute term: the review's re-run put `batch/router-ecc83-input` at 1.069x against a 1.06897x floor — a **2 ms** difference on a 29 ms stem, one hair from a REGRESSION (S2) | `port-sha` gains the five generators' `+uncommitted-changes-under-crates` marker and T0 is re-stamped. The floor becomes `max(1.05, 1 + prior_spread/prior_cpu, 1 + current_spread/prior_cpu, 1 + 0.005s/prior_cpu)`, documented as a formula in the script header; `equal_quality` now initialises from *whether a quality baseline exists* rather than to `True`; `read_tsv` takes its columns only from a named `# cols:` header and treats a missing one as a hard error (S6); `check_gate` refuses an unversioned baseline (N6); a failed or timed-out `cpu_s` repeat fails the row instead of contributing its burnt CPU to the median (N7). |
| A21 | 1, and every later task that regenerates | Task 1 step 5: "**Regenerate G, R, B, C, D from the port**, once, at the end of the task", with a `goldens moved:` line predicting "batch SES bytes move on 8/8 stems, router JSONL on 6/6 rows, CLI on 13/13 stems" because "#234 … so every multi-pass stem is tightened further" | **Nothing moved.** Task 1 ran the full regeneration and measured it: router JSONL **0/6** rows, `roundtrip.dsn`/`unrouted.ses` **0** files, `batch.ses` 7/8 and `route.ses` 12/13 moved by **exactly two lines each** — quirk #92's `hostCad`/`hostVersion` → `host_cad`/`host_version` — and **all 13 manifests unchanged** in `normalized_score`, `trace_length_mm`, `via_total`, `bend_count` and `incomplete_count`. Corpus-median `cpu_s` ratio **1.000**, so BP4's escalation could not fire either. Structural: every parity driver already ran `RouterBudget::disabled()`, so B and R could not move; the CLI lane is the only one that ran the clock live and its stems never trip it (the register's own **0 trips on all eight whole-board stems**); and #208 and #224 are unreachable from the corpus — **no stem in any of the three fixture tables sets a neck width or any timeout**. What was left was a pure lane switch that broke **16 tests**, fifteen repairable and one not: regenerating drops the count of `hostCad`-bearing files under `tests/reference/` from **20 to 0**, and `crates/fr-core/tests/stats.rs`'s "the one file shape where the host scrape SUCCEEDS" (row 31 — quirks #248(b), #250, #252) reads that file's *bytes*, so the fixture **is** the assertion and no literal can stand in for it; BL8 forbids reading the frozen copy to restore it | **BT.** (1) **No regeneration at Task 1**; step 5 closes as *measured zero golden movement; regeneration deferred to first actual movement per family*. (2) **The G1 cadence is amended**: a family regenerates only when a fix **moves** it, and the #92 lane switch lands per family the first time that family regenerates for cause. (3) The `stats.rs` case is **pre-resolved**: at that future regeneration the `hostCad`-bearing reference files **migrate to committed directed fixtures under `crates/fr-core/tests/data/`** with a provenance header naming them as the pre-lane-switch real-corpus files and the commit they were cut at, and the `p8t2` transcript is re-cut against them in the same commit — BL8 intact, quirk exercise survives. Task 1's own closing artefacts are unaffected: the freeze, the jar column, `quality-ab-T1.tsv` and the two-run identity check all landed. |
| A20 | 0, 1 | (not stated) — nothing executed the `--from-port` lane | `crates/fr-dsn/tests/parity_dsn.rs` bound the *call pair* `refwriter` makes, not the binary, and `refwriter.rs` carried a **third** copy of `design_name`; a drift would have surfaced at Task 1's regeneration as an unexplained golden churn rather than as a failing test (Task 0 review, S5) | `design_name` deduplicated into **`parity::dsn_design_name`**, called by both `parity_dsn.rs` and `refwriter.rs`; and `parity_dsn.rs::the_refwriter_binary_reproduces_a_committed_reference` **builds and runs the binary** on `Issue143-rpi_splitter` and byte-compares both outputs against the committed references. `FR_REFWRITER_BIN` skips the build for a caller that has one. |
| A22 | 3 | Task 3's acceptance: "**C moves on the 2 KiCad stems** (`cli-kicad-ecc83-json`, `cli-kicad-complex-hierarchy-json`): their `route.json` now holds a routed board", with a `goldens moved:` line predicting the wire count rising from 0 to the SES's, and a Commit 4 that re-cuts them | **There is no `route.json` under `tests/reference/`, and there never was.** `grep -A1 -h -- "-do" tests/reference/cli-*/argv.txt` answers `<OUT>/route.ses` thirteen times out of thirteen; `find tests/reference -name '*.json' | grep -v manifest` answers the eight DRC reports and nothing else. The two KiCad stems are named for their *input* — `-de …Issue649-kicad_ecc83….json` and `-de …Issue733-kicad_complex_hierarchy….json` — and both write a Specctra session, so quirk #289, which lives entirely on the `-do out.json` **output** path, is unreachable from the corpus. Neither are the other two: #265's delete never fires because every generator run writes into a fresh `<OUT>`, and #268's refusal never fires because every stem's extension is `.ses`. Measured at both ends of the task: `git status --short -- tests/reference` is empty across all four commits | **Ruling BT, applied as written**: a family regenerates only when a fix moves it, and none does. `goldens moved: NONE`, and **neither the #92 `hostCad` lane switch nor BT's pre-resolved `stats.rs` fixture migration lands at Task 3** — BT attaches both to the first family that moves for cause, and that is **Task 2**, which carried them with its B/C regeneration at `bd296d7` (`hostCad`-bearing files under `tests/reference/` went 20 → 0 there, and the migrated fixtures are `crates/fr-core/tests/data/p8t2-batch-ses/`). Task 3 rebased onto that tip without a single conflict and without moving a reference byte: `git diff --stat 5152291..HEAD -- tests/reference` is empty. The fixes are bound by four directed tests instead, and by `p8t7`'s rung (c), which is **rewritten** rather than re-golden: it asserted jar-equals-port on exactly the behaviour #289 changes, and now answers **XDIFF** while measuring that the jar still has the quirk (same 0-trace document at `-mp 1` and `-mp 8`), that the port does not, and that the port's JSON is the board its own SES describes. G2's "no routed board changes" is asserted rather than assumed: 377 quality cells against T1's tsv, **0 moved** |
| A23 | 4 | Task 4's interfaces line: "`BoardReadResult::Partial { board, diagnostic }`" — two fields | every real consumer of a `BoardReadResult` needs more than the board: `fr_core::load::parse_board_result` answers `Err("the reader produced a board without a coordinate transform")` unless the variant carries one, and the CLI surfaces the reader's `warnings`. A two-field `Partial` would therefore be a **hard failure at every consumer**, which is exactly what the roadmap's correction of the register's binary framing forbids | **Filed by the implementer.** The committed variant carries the same four payload fields as its `Success` sibling — `board`, `metadata`, `warnings`, `coordinate_transform` — **plus** `diagnostic`. The interface line's intent (a third state carrying the partial board *and* the diagnostic) is met; only its field list is wider. Recorded on the variant's own doc comment. **Renumber check, resolved at landing:** Task 3's A22 landed at the end of this table and this row rebased onto it as a conflict in exactly that slot; both are kept, in numeric order, and no shift was needed. |
| A24 | 4 | Task 4's Files line names `crates/fr-dsn/tests/{rules.rs,network.rs}` and `crates/fr-dsn/src/lexer/scanner.rs`, and its Tests block binds `tests/rules.rs::a_rules_file_naming_an_absent_layer_leaves_the_default_width_untouched` and `tests/network.rs::a_component_with_an_absent_padstack_is_rejected_whole` | **none of those three files exists.** The tree has `crates/fr-dsn/tests/rules_round_trip.rs` and `crates/fr-dsn/tests/network_scope.rs`, and the scanner is `crates/fr-dsn/src/lexer/mod.rs` (`lexer/tables.rs` beside it is the generated DFA). Creating two new test files next to the existing siblings would have split one subject across two files for no reason | **Filed by the implementer.** The **test names** are what binds and all of them are present, under those names, in the existing sibling files: #112's in `rules_round_trip.rs`, #86's in `lexer.rs`, #95's and #94's in `structure_scope.rs`, #90's and #91's in `scopes.rs`, #93's in `geometry_scopes.rs`. #103's went one level deeper still — a `#[cfg(test)]` module **inside** `crates/fr-dsn/src/parser/network.rs` — because `insert_component` is private *and* the defect is unreachable through the public reader (`Library.readScope` refuses the whole file for the same condition first), so the dangling padstack id has to be installed on the board directly; the justification sits on the module. **Task 22's brief should say `lexer/mod.rs`**, since ruling BP6 names this file at both ends. |

---

## Plan self-review

**This plan has been amended once, before dispatch, and the self-review below is written against the amended text.** A pre-flight conflict scan (147 rows; 27 distinct CONFLICT and 18 distinct AMBIGUOUS findings after deduplication) read the plan against the tree at `eff706a`, and controller rulings **BP1–BP19** resolved every finding. **The single largest defect it found was that the measurement spine had been written without reading the bench harness** — it named an output directory nothing writes to, passed a comma-joined tier list where `corpus.py::select` takes exactly one tier name (selecting zero boards), asked for three seeds where both runs of record used one, and left `--runs` off the compare, so it would silently have taken "the latest run per candidate" instead of the frozen java view. The G3 block, all three milestones and Task 1's freeze are now written from `benchmark/scripts/remote-run.sh`, `benchmark/bench/cli.py` and `benchmark/results/`'s two runs of record, and the amendment log above records each correction with what the tree actually said. **What the scan verified and this amendment preserved**: the 121/121 recount, the freeze *ordering* (Task 0 regenerates nothing; the freeze is Task 1 commit 1; the first regeneration is Task 1's last), the three milestone placements (Tasks 2, 16, 18 — and 18 is the last behavioural task), Task 2's M1 acceptance triple (clean pass ≥ 0.77 / connected ≥ 0.79 / DRC-clean ≥ 0.95, which **matches `benchmark/reports/java-regressions-2026-09.md` exactly** and silently corrects the survey §4.1's self-contradictory ordering of the same three numbers), and Task 2's `board_rules.rs:138` vs `board/mod.rs:1787` disambiguation, **verified correct in the tree — both methods exist at exactly those lines**. **The fix lists are unchanged: 121 rows, no row added, none removed, none moved between tasks.**

**Spec coverage — every FIX row of the survey's §3–§6 appears in exactly one task.** Counted row by row against the survey's tables:

| survey section | FIX rows | tasks |
|---|---|---|
| §3.1 output file | 3 | **3** |
| §3.2 non-termination and OOM | 5 | **5** (#71+#76+#106, #105), **8** (#162), **4** (#86), **12** (#27) |
| §3.3 silent data loss | 7 | **4** (6), **5** (#211+#45) |
| §3.4 reachable crashes | 6 | **6** |
| §3.5 KiCad JSON | 5 | **7** |
| §4.1 the two regressions | 4 | **2** |
| §4.2 optimizer and pass loop | 7 | **9** |
| §4.3 rooms and doors | 10 | **8** (8), **16** (#192), **17** (#193) |
| §4.4 obstacle and clearance | 8 | **10** |
| §4.5 board geometry | 10 | **11** |
| §4.6 polygon and circle | 1 | **12** |
| §4.7 airlines and incompletes | 5 | **13** |
| §4.8 stale caches | 2 | **14** |
| §4.9 fanout and orderings | 5 | **15** (3), **18** (#44+#63+#74, #210) |
| §4.10 via optimizer | 1 | **16** |
| §4.11 determinism and the clock | 3 | **1** |
| §4.12 policy switches | 4 | **18** |
| §5 DRC and report accuracy | 12 | **19** (11), **20** (#291) |
| §6.1 settings merge engine | 10 | **21** |
| §6.2 the command line | 6 | **22** |
| §6.3 manifest and statistics | 6 + #281's sub-item | **20** (6), **7** (#281's `OutlineJson.clearance`) |
| **total** | **121** | **Tasks 1–22; Tasks 0, 23, 24, 25 carry none by design** |

**The count, reconciled.** The survey's §12 headline says **117 FIX rows** and its §7.5 table sums to the same number, but neither sub-list sums to its own subtotal (§7.5's "one family: 49" enumerates 27). A strict row-by-row enumeration of §3–§6 — 129 status-bearing table rows, minus 4 `ALREADY-FIXED`, minus 3 pure `NOT-A-FIX`, minus the 2 rows cross-listed twice (#192 in §4.3 and §4.10; #155 in §5 and §6.1) — gives **120 distinct FIX rows**, plus §6.3's `OutlineJson.clearance` sub-item that survey §10.3 places in group 7, for **121**. **All 121 are placed; none is unplaced.** The three-row gap against the headline is the survey's own arithmetic, not a missing row, and it is recorded here so a reader who counts gets the same answer twice. **The pre-flight scan re-ran this count independently and confirmed it row by row** — 121 fix-list rows across Tasks 1–22, every task's declared count equal to its actual count, 199 distinct register ids placed plus the four new #293–#296, and exactly one id in two tasks (#9, the deliberate BM5 split, named at both ends). **The survey's §12 headline of 117 stays on the record as a survey defect**, so that a later reader does not "correct" the plan back to it.

**One row the survey's group table omitted, placed here:** **#208** (§4.11, `retryConnectionNecked` re-uses the spent `TimeLimit`) appears in no group of §10.3. It is placed in **Task 1** with §4.11's other two rows — same section, same subject (the clock), same regeneration.

**Two rows cross-listed by the survey, placed once each:** **#192** is stated in §4.3 and restated in §4.10 "because it belongs to the same drill-page task" — placed in **Task 16**, and Task 8's list says so. **#155** is stated in §5's mixed `#149/#150/#155` row and again as its own §6.1 row — placed in **Task 21** (it is a settings row and is gated on #115), and Task 19's list says so.

**Placeholder scan.** No "TBD". No "similar to Task N". No "add error handling". Every task names its files, the register ids it closes with their mechanism and fix sketch copied from the survey, the interfaces it consumes and produces, its named binding tests, which golden families regenerate and in which direction, its gate tier, its steps and its commit message. **Nine places name a decision the implementer must take rather than one this plan takes, each with the default already chosen and the deciding evidence named:** #94's fixture (check `cli-large-outline` first, build one otherwise — Task 4); #217's rank break (default: delete the branch — Task 9); #192's boundary case (default: a touching shape reaches the next page — Task 16); #281's `OutlineJson.clearance` (default: wire it — Task 7); #291's field meaning (default: test `DrillItem` first and keep all three — Task 20); #236 (default: do not port — Task 20); #148 (default: drop `Comparable` — Task 13); #155 (default: read the flags — Task 21); and each of Task 18's six policy defaults (current behaviour, decided at M3). **Two things this plan deliberately does not decide** and says so: whether the 500 µm board-edge keep-out should apply by default at all (#231 — measured as a **stem A/B at Task 16**, not a bench arm; a corpus number for it would need a controller-authorised fourth run, which this plan does not authorise — ruling BP3), and what to do about #193 (Task 17 produces the recommendation; acting on it is a later decision).

**Interface consistency across tasks.** `RouterBudget::default()` changes once, in Task 1, and every later task consumes the new default. `BoardReadResult::Partial` is introduced in Task 4 and consumed by Task 7's diagnostics. `RuleLayerScope` is introduced in Task 4 and read by Task 21's #142. `calculate_item_distance` is introduced in Task 2 and its element type is widened once, in Task 9's #213 — **never re-declared**. `BatchLoopExit` is introduced in Task 9 and read by Task 20's manifest. `fr_board::BoardRules::get_min_trace_half_width` is **consumed, never changed**, and Task 2 pins which of the two same-named methods is meant. **`ScoringSettings`' non-optional weights and the manifest guard are BOTH Task 21's** (ruling BP14, correcting this paragraph): #258 makes the weights non-optional **and** drops `from_job`'s guard, in Task 21, and **Task 20 runs first** — Task 20 therefore has **no** forward dependency on Task 21, and an implementer must not read one into it. Task 20 touches `manifest.rs` for its own seven fields; Task 21 touches `from_job`'s guard. The task bodies were always right; this sentence was backwards. `Line`'s identity token has exactly two readers, closed in Tasks 14 and 18, and Task 24 records the outcome either way. `JavaTreeSet` is deleted in Task 24 and **only** in Task 24 — no earlier task may delete it, because deleting it before #160/#161 and #171 land would silently drop elements.

**What could still go wrong, in order of cost.** **(1) M1 does not reproduce the report's ablation numbers** — the ablation was measured on the jar and the port is a port; a gap is a real difference and must be root-caused, not tolerated. Mitigated two ways: the port's *pre-fix* position is already measured (the **`v1.0.0-rs`** run of record, on this branch's own base commit — no new run needed), and **ruling BP12 gives the miss an exit**: the deliverable becomes a bisected root-cause report and the controller adjudicates revert / rework / accept-with-gap, then M1 re-runs once. **(2) Task 9's #227 makes boards worse** — an optimizer that finally runs could, in principle, optimise toward the wrong objective; mitigated by the direction check (score must rise on every stem) and by the fact that #227 is committed alone. **(3) A golden churn nobody can explain** — mitigated by `run.sh --against-jar` (Task 0) and by the frozen baseline, which together let any surprising diff be triaged against the jar without re-introducing it as a gate. **(4) Task 22's CLI ramp gets the deprecation window wrong** — mitigated by making the ramp explicit, dated and documented rather than implicit. **(5) The plan is too long to finish** — mitigated by BL2's tiering: the expensive gate runs three times, not 121 times, and every task is independently mergeable with its own evidence. **(6) A speed regression lands invisibly** — mitigated by ruling BP4: `cpu_s` is a G2 column against a **rolling port** baseline, equal-quality slowdowns are rejected outright, and Tasks 1, 9, 12 and 13 are named in advance with a pre-authorised escalation path so the four expected offenders produce a ruling rather than a surprise at M3.

**The non-FIX rows, accounted for so none looks dropped.** The survey's §3–§6 also carry **4 `ALREADY-FIXED` rows** — #241/#244/#250/#252/#257 (five Java hangs the port already answers with a value), #61/#30/#75 (three orderings the port already answers better, **and which must not be "restored"** — #30 and #61 stay preconditions for any future parallelism), the deadline errata (`c3a7ee0`), and #144/#145 (hash order and locale) — plus **5 `NOT-A-FIX` rows**: #279+#277 (`readBoard`'s `catch (Throwable)` and its prose `ParseError.detail`), #216/#225/#226 (dead arms, deleted alongside #228 in **Task 9**), #149/#150 (the hard-coded `focusNets` debug block and the array-instead-of-count log line, **not ported**), the fourteen-row unported cluster #237-#240/#243/#245/#253/#264/#266/#270/#276/#281/#292, and the bisect note (`cbca03ee`/`6182f236`). **None owes code.** Where one has a live consequence it is carried by a task and named there: #216/#225/#226 by **Task 9**, #281's `OutlineJson.clearance` by **Task 7**, #144/#145's disappearing JVM flags by **Task 19** and Task 25's report, #250's totalization by **Task 20**, and the bisect note's surviving DRC-side grouping logic by **Task 19**'s unconnected-items rows. The survey's **10 KEEP rows** (§9.1) are seeded `keep` in the register by **Task 0** and are the rows this plan is forbidden to "fix": #62, #83, #92, #202's three-state stop, #273's order, #281's four DTO fields, `normalized_score` as `f32`, the six accepted divergences (#12/#14/#19/#20/#10/#8), the 165 `// Java bug:` markers, and §8's KEEP shims.
