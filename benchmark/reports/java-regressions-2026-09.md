# Two routing regressions in freerouting between v2.1.0 and HEAD (2.3.1-SNAPSHOT)

**status: fixed in the port at Plan 9 Task 2.** Both regressions below are repaired in
`freerouting-rs` — Regression 1 as register row **#293** (the airline-distance ordering of the
work list, restored in `crates/fr-router/src/pipeline/batch_autorouter.rs` over the three
`AutorouteAirlineCalculator` methods `crates/fr-router/src/pipeline/airline.rs` now ports) and
Regression 2 as row **#294** (the micro-neckdown fanout fallback, floored at
`BoardRules.getMinTraceHalfWidth()` in
`crates/fr-router/src/autoroute/path/inserter.rs`). Neither is fixed **upstream in Java**: both
rows keep their "Java-side fix owed" cell in `docs/java-quirks.md` and are recommendation 8's two
standing pull requests. The two secondary findings this file opens are now register rows too —
**#295** (the every-4th-pass exhaustive tree, measured after #159 lands in Task 8) and **#296**
(via inflation, watched at every Plan 9 milestone) — and each carries its open question and its
owner there.

Measured with the benchmark suite in `benchmark/` on 605 PCBench boards (real, human-routed
KiCad designs whose references are routing-DRC-clean and fully connected under their own design
rules), judged by KiCad 10 DRC with each board's own rules, `-mp 10`, 5-minute cap, one isolated
config dir per run, single-threaded. The router is deterministic, so every delta below is exact,
not statistical. Full data: `reports/pcbench-freerouting-2026-08-28.md`; per-board artefacts for
both regressions were verified by ablation builds at HEAD.

v2.1.0 is the high-water mark: on the 138-board small tier it fully connects 0.81 of boards with
0.92 DRC-clean (clean pass 0.76). HEAD measures 0.76 / 0.73 (clean pass 0.60). The loss decomposes
into exactly two independent changes.

## Regression 1 — connectivity: airline-distance net ordering removed (first bad: `933d2980`)

**What changed.** Commit `933d2980` ("Remove useSlowAlgorithm parameter from autorouter",
2026-01-14, shipped in v2.2.0) removed two behaviours in one commit:

1. the shortest-airline-first ordering of the items to route —
   `autoroute_item_list.sort(Comparator.comparingDouble(this::calculateItemDistance))` was
   commented out with the note "Disabled in v2.3 because it negatively impacts convergence
   compared to v1.9 (natural order)";
2. the exhaustive ("slow") search tree that previously ran on every 4th pass
   (`useSlowAlgorithm = p_pass_no % 4 == 0` → `false`).

**Measured effect.** Fully-connected rate on the small tier drops 0.81 → 0.75 at v2.2.0 and never
recovers; 17 probe boards that v2.1.0 completes are left unrouted by every 2.2.x release. The
commit's own justification is contradicted by the data: restoring the sort alone at HEAD (a
one-line ablation; `calculateItemDistance` still exists in `AutorouteAirlineCalculator`) brings
connectivity back to exactly v2.1.0's level (0.76 → 0.82) and is slightly faster. On medium/large
boards the sort is neutral within noise, so a conservative fix is to enable it at least for boards
under ~50 nets — or unconditionally, since no tier measured worse with it.

**Bisect note.** A first bisect landed on `cbca03ee` ("Improve unconnected items detection in
DRC", 2026-01-10), which changed Trace/DrillItem contact detection to a Manhattan tolerance of
`half_width + 1`. That tolerance broke the same probe boards but was reverted before v2.2.0
shipped (`6182f236`); re-bisecting with the tolerance neutralised at every step isolates
`933d2980` as the change that ships in releases. `cbca03ee` is still worth review: its
tolerance-based contact test briefly made the router treat nearly-touching items as connected
(and its DRC-side grouping logic survives at HEAD).

## Regression 2 — DRC: micro-neckdown fanout fallback ignores minimum track width (first bad: `f31a0c84`)

**What changed.** Commit `f31a0c84` ("Add micro-neckdown fanout fallback for trace insertion",
2026-05-19, shipped in v2.3.0) retries a failed 2-point fanout insertion at reduced widths — the
pin's neckdown width, then 75 % and then 50 % of the class width — "keeping the same clearance
class", with no check against the board's minimum track width. (The opt-in `neck_width_um` retry
from `9b782275` is off by default and not involved.)

**Measured effect.** On any board whose net-class width equals its minimum width (very common),
the fallback produces sub-minimum traces: `track_width` violations on 31/146 small boards,
DRC-clean rate 0.94 → 0.73 (small tier) and 0.93 → 0.40 (large tier). Because a clean-pass metric
weighs a DRC violation like an unrouted net, the fallback converts "one net open" into "board
fails DRC" — and on the small tier it recovers no connectivity at all (0.76 with or without it).
Its benefit is real only on large boards (fully-connected 0.17 → 0.33), so the fix is a guard,
not a revert: skip retry widths below the board's minimum track width (`min_track_width`, or the
class width when the class is at the minimum).

**Ablation proof.** HEAD with the fallback disabled: DRC-clean 0.95, best of any build measured.
HEAD with the fallback disabled **and** the airline sort restored: clean pass 0.77 / connected
0.79 / DRC-clean 0.96 on the small tier — better than v2.1.0 (0.76 / 0.81 / 0.92) — and on the
large tier clean pass 0.30 vs v2.1.0's 0.20. Two one-line changes recover, and slightly exceed,
four releases of regression.

## Secondary findings (not root-caused, worth issues of their own)

- **Via inflation:** between v2.2.4 and v2.3.0 the router's via usage roughly doubles
  (0.37× → 0.71–0.94× of the human reference count) alongside a ~25 % slowdown — the "recovery"
  work after the ordering regression appears to try harder with more vias.
- **Self-report vs DRC disagreement:** the in-process statistics (`normalized_score`,
  incomplete/violation counts) disagree with the router's own DRC-only mode (`-de in.dsn out.ses
  -drc`) on ~45–63 % of boards (e.g. per-pin vs per-pair hole-clearance counting); external
  scoring should not trust the manifest numbers.
- **`passes_completed` misreports 1** in `result.json` even when logs show 18–30 passes.
- **2.2.x CLI splits `-de` paths on spaces**, failing to load any file whose path contains one
  (fixed by v2.3.0).
- **Crash on non-DSN input:** a binary file passed to `-de` dies with an NPE in
  `RouterSettings.applyBoardSpecificOptimizations` instead of a parse error.

---

# Status appendix — the port's own measurement of both fixes (Plan 9 Task 2)

Everything above was measured **on the jar**, over 605 PCBench boards, by ablation builds. This
appendix records what the same two fixes did **in the port**, on the 29-stem G2 corpus, at Plan 9
Task 2. That corpus is far smaller and much coarser — 21 routed stem *rows*, eight boards of which
appear in both the batch and CLI families, several of them deliberately truncated to `-mp 2` — so
it is a **direction** check and not a re-measurement of the tiers above. The 605-board M1 run
against the frozen `java-278fe14` view is the acceptance bar and is run separately.

Two committed artefacts, both cut at commit `bd296d7`:

    benchmark/baselines/ab/quality-ab-T2.tsv          R1 + R2
    benchmark/baselines/ab/quality-ab-T2-r1-only.tsv  R1 only (R2's guard removed, nothing else)

The console output of both runs is `scripts/differential/out/quality-ab-T2.txt`, which is not
committed — that directory is ignored — because the tsvs carry every number a reader needs.

## What moved, and which fix moved it

Per routed board, `incomplete` / `violations`, both from the referee's DRC document and never from
the manifest. `T1` is the port immediately before this task. A board that appears in both families
is listed once; the two families agreed on every quality column in all three runs.

| board | T1 | R1 only | R1 + R2 |
|---|---|---|---|
| `router-rpi-splitter` | 0 / 0 | 0 / 0 | 0 / 0 |
| `router-dac2020-bm01` | 31 / 0 | 32 / **48** | **38** / **0** |
| `router-j2-reference` | 0 / 0 | 0 / 0 | **1** / 0 |
| `router-tutorial-board` | 0 / 0 | 0 / 0 | 0 / 0 |
| `router-ecc83-input` | 0 / 0 | 0 / 0 | 0 / 0 |
| `router-fanout-bm11` | 2 / 5 | 3 / 4 | **6** / 4 |
| `router-strict-drc-cnh` | 11 / 17 | **14** / 17 | 14 / 17 |
| `router-empty-board` | 0 / 0 | 0 / 0 | 0 / 0 |
| `tutorial_board` | 0 / 0 | 0 / 0 | 0 / 0 |
| `Issue026-J2_reference` | 0 / 0 | 0 / 0 | **1** / 0 |
| `large-outline` | 8 / 415 | **7** / 419 | **11** / 423 |
| `kicad-ecc83-json` | 6 / 14 | 6 / 14 | 6 / 14 |
| `kicad-complex-hierarchy-json` | 0 / 36 | 0 / 36 | 0 / 36 |

**R1's own account.** It costs one connection on `router-dac2020-bm01`, one on
`router-fanout-bm11` and three on `router-strict-drc-cnh`; it *wins* one on `large-outline`; and it
leaves the other eight routed boards exactly where they were. It also puts 48 `via_dangling`
violations and 45 extra vias on `router-dac2020-bm01`, which is this corpus's clearest instance of
the via inflation §"Secondary findings" describes.

**R2's own account.** It removes the sub-class-width traces the fallback used to emit. The
`neckdown_below_class_width` column, new in this tsv (ruling BP15, owed to Task 14), falls from
**372 wire segments across the 21 rows to 2**:

| board | R1 only | R1 + R2 |
|---|---|---|
| `router-rpi-splitter` | 2 | 0 |
| `router-dac2020-bm01` | 49 | 0 |
| `router-j2-reference` | 43 | 0 |
| `router-fanout-bm11` | 62 | 0 |
| `router-strict-drc-cnh` | 1 | 1 |
| `large-outline` | 15 | 0 |
| `Issue026-J2_reference` | 43 | 0 |
| every other routed board | 0 | 0 |

That is the report's "very common" case, confirmed in the port: on almost every one of these
boards the net-class width **is** the design-rule minimum, so the whole fallback is skipped and
those connections fail honestly instead. It costs six connections on `router-dac2020-bm01`, three
on `router-fanout-bm11`, four on `large-outline` and one on `J2_reference` — and on
`router-dac2020-bm01` it takes the 48 `via_dangling` violations R1 introduced back to **zero**,
along with 56 of the vias.

**The `track_width` half of Regression 2 is invisible on this corpus, and that is a gap in the
referee rather than in the fix.** The port's DRC has no track-width rule at all — `grep -rn
track_width crates/fr-drc` answers nothing — so the violations this guard prevents are counted
nowhere in the tables above. On the jar, judged by KiCad DRC, they are 31/146 of the small tier
and the difference between 0.94 and 0.73 DRC-clean. Any reading of R2 from this corpus alone sees
its whole cost and none of its benefit.

## Where the port lands overall

Summed over the 21 routed stem rows:

| | incomplete | violations | trace length | vias | bends | sub-class-width segments |
|---|---|---|---|---|---|---|
| T1 | 102 | 509 | 27 291.3 mm | 504 | 2 883 | not measured |
| R1 only | 111 | 607 | 26 300.7 mm | 599 | 2 959 | 372 |
| R1 + R2 | **136** | **515** | **25 480.8 mm** | **481** | **2 728** | **2** |

Against T1, the pair is **-6.6 %** trace length, **-4.6 %** vias, **-5.4 %** bends, a
corpus-median `cpu_s` of **0.969×** and clearance violations still **0 on every routed stem** —
against **+34** incomplete connections. Against R1 alone, R2 is **-92** violations, **-19.7 %**
vias and 370 fewer sub-minimum segments for **+25** incompletes. Shorter, cheaper, faster boards
with more honestly-open nets.

## I2 (#296) — the via column, measured

The hypothesis in §"Secondary findings" is that the v2.2.4 → v2.3.0 via doubling is the
*downstream compensation* for the ordering regression rather than a defect of its own. The port's
own numbers point the same way once **both** fixes are in: 504 vias at T1, **599** with R1 alone
(+18.8 %), **481** with R1 + R2 (-4.6 % against T1). R1 alone inflates the count exactly as the
hypothesis predicts (`router-dac2020-bm01` 81 → 126, `large-outline` 162 → 169,
`router-j2-reference` 16 → 18); R2 deflates it past where it started — `router-dac2020-bm01` 126 → **70**, below T1's own 81, and
`router-fanout-bm11` back within two of it. So on this corpus the inflation does **not** survive the pair,
which is the answer this file asked for — recorded as a *direction*, because 21 rows are not 605
boards and the M1 run against the `v1.0.0-rs` run of record is what settles it.

## The real-world case: `screen-keys`

`benchmark/docs/plan-9-prep/fixtures/real-world-screen-keys/` is a 71-connection board on which
both `freerouting-rs` v1.0.0 and the Java HEAD jar leave the same single 5V connection open. Its
`EVIDENCE.md` assigns the hypothesis to the door-drop family (#160/#161/#163, Plan 9 Task 8) and
predicts 71/71 after those land. Run here at `-mp 10`, before and after, because the data point is
worth having either way:

| build | incomplete | normalized score | vias | trace length |
|---|---|---|---|---|
| base (neither fix) | **1** of 71 | 985.9063 | 5 | 1964.8478 mm |
| R1 only | **3** of 71 | 957.738 | 5 | 1804.3386 mm |
| R1 + R2 | **3** of 71 | 957.738 | 5 | 1804.3386 mm |

**The 5V corridor does not close**, which is what `EVIDENCE.md` predicted and is Task 8's to
answer. R2 is exactly neutral here — this board has no fanout neckdown to remove. R1 costs it two
further connections while shortening its copper by 8 %; that is a *new* data point for Task 8,
which should now re-measure against the base row above rather than against the v1.0.0 numbers in
`EVIDENCE.md`.

## The release lane's 14 jar-parity tests, and which fix moves each

The port's `#[cfg_attr(debug_assertions, ignore)]` tier — the release-only lane that routes whole
corpus boards and compares them against JVM probe transcripts — carries **14** assertions that
these two fixes move. All 14 are in `crates/fr-router/tests/`, and every one is a literal
transcribed from a `p7t8`/`p7t9` run over a board whose routing order R1 changes:

| test | R1 | R2 | what moved |
|---|---|---|---|
| `batch_loop::a_normal_finish_reports_cancelled_not_finished` | ● | | pass count 1 → 2 |
| `batch_loop::a_full_history_never_ranks_a_board_past_its_cap` | ● | | history length 3 → 2 |
| `batch_loop::only_a_pass_that_routes_nothing_reaches_finished` | ● | | `Finished` → `Cancelled` |
| `batch_loop::max_passes_zero_is_unlimited` | ● | | `Finished` → `Cancelled` |
| `batch_loop::the_rank_the_loop_tests_is_read_after_restore_boards_reorder` | ● | | the fixture can no longer make three strictly increasing scores |
| `optimizer::a_near_perfect_board_exits_before_the_first_pass` | ● | | no pass completes |
| `optimizer::an_auto_router_only_stop_leaves_every_item_rejected` | ● | | items visited 6 → 5 |
| `optimizer::consecutive_failures_break_the_pass` | ● | | 7 → 5 |
| `optimizer::the_optimizer_stage_matches_the_jvm` | ● | | passes 2 → 1 |
| `optimizer_items::the_item_sequence_matches_the_jvm` | ● | | six items → five, and every id moved |
| `optimizer_items::an_unimproved_item_restores_the_clone_byte_for_byte` | ● | | first item id 43 → 86 |
| `fixtures::dac2020_bm01_pipeline_one_pass_leaves_at_most_56_incompletes` | ● | | 56 → **69** |
| `fixtures::dac2020_bm01_pipeline_two_passes_leave_at_most_28_incompletes` | ● | | 28 → **37** |
| `fanout_order::fanout_escapes_every_smd_pin_of_the_corpus_board_like_the_jvm` | | ● | escaped pins 11 → 10 |

Attributed by ablation, not by inspection: with **both** fixes removed and nothing else changed,
the five affected test binaries answer **90 of 90 passing**; with R1 alone, 13 of the 14 fail; the
fourteenth is R2's, and it is R2 working exactly as specified — one SMD pin can no longer escape,
because escaping it needed a sub-minimum trace.

Two of these are quality bounds rather than transcripts, and they are the sharpest single number
in this appendix: `Issue508-DAC2020_bm01.dsn` was asserted to leave **at most 56** connections open
after one pipeline pass and **at most 28** after two; it now leaves **69** and **37**. That is R1's
cost on the corpus's hardest board, stated without a metric in between.

The other twelve are `matches_the_jvm`-style literals, and re-cutting them is not a mechanical
edit: a test named "matches the JVM" over a routed board cannot be made true again by changing its
numbers, because the port has deliberately stopped matching the JVM there. Re-basing them on the
port — with R1/R2 named as the cause, exactly as the B and C reference families were re-based —
is the work the **accept** branch of the adjudication below implies, and it is deliberately not
done here: if the adjudication is *revert* or *rework R1*, all twelve revert with it and the
evidence above would have been erased to produce them.

## The gate this did not pass, and why it is an escalation rather than a failure

`scripts/quality-ab.sh`'s G2 rule is "incompletes must not rise on any stem". Seven of the 21 rows
break it, and **R2 breaks it by design**: its whole content is that a connection which can only be
completed with a sub-minimum trace should fail instead. The rule was written for the parity era,
when a rise meant the port had diverged from the jar; it cannot express a trade of connectivity
for legality, and the ablation above says that trade is worth 0.73 → 0.95 DRC-clean on the small
tier. R1's own three rises are a different matter: on this corpus they are a real per-stem cost,
against a 605-board ablation that calls the same change a corpus-level win.

Both are ruling BP12 escalations with a named next step. The numbers, the attribution and the
artefacts are here; the milestone M1 run over 605 boards is the measurement the fixes were
designed against; the controller adjudicates between them.
