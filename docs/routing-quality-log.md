# Routing quality investigation

Baseline: `main` at `636e039ea3a2`; worktree `../copperroute-routing-quality`, branch
`routing-quality`. The separate neckdown commit `7f5454b` was read, not applied.
Existing main checkout and workbench edits are preserved.

## Completed full-run comparison

Recomputed from exports and successful referee reports. Each run attempted 845
boards. Comparisons include only boards scored in both runs; missing and newly
scored outputs are separate. U = unrouted; V = benchmark routing violations.
CPU ratio <1 is faster.

| Candidate | Referee | Pairs | U improved / regressed | ΔU | DRC gainers | ΔV | CPU ratio |
|---|---|---:|---:|---:|---:|---:|---:|
| nominal | kicad | 739 | 122 / 56 | -271 | 30 | -1 | 0.8984 |
| nominal | java-drc | 101 | 38 / 4 | -248 | 1 | +67 | 0.9678 |
| images | kicad | 739 | 14 / 10 | +71 | 6 | -25 | 1.0005 |
| images | java-drc | 101 | 8 / 6 | -73 | 1 | +1 | 1.0057 |
| nominal-smoothing | kicad | 739 | 124 / 52 | -415 | 30 | -7 | 0.8778 |
| nominal-smoothing | java-drc | 100 | 37 / 4 | -263 | 2 | +68 | 0.9678 |
| smoothing | kicad | 739 | 13 / 13 | +56 | 2 | -1 | 1.0078 |
| smoothing | java-drc | 101 | 10 / 5 | -62 | 0 | +0 | 1.0117 |
| census | kicad | 740 | 12 / 3 | -181 | 2 | +16 | 0.9894 |
| census | java-drc | 101 | 10 / 0 | -216 | 0 | +0 | 0.9842 |

Supplementary KiCad errors and output availability:

| Candidate | Δ mask errors | Δ all errors | Newly unscored outputs |
|---|---:|---:|---|
| nominal | +333 | +332 | Djinn |
| images | +21 | -4 | Djinn |
| nominal-smoothing | +309 | +302 | issue070, Djinn |
| smoothing | -14 | -15 | Djinn |
| census | -6 | +10 | None |

Four baseline Java fixtures were unscored and are excluded from paired totals.
Census newly scores issue756-tomu-fpga8: 11 U / 853 V / 342.12 CPU seconds;
this is an output recovery, not a measured improvement against a scored baseline.
Exact data: [routing-quality-results.json](routing-quality-results.json).

Census passed the final workspace gate and is selected for landing: matched -397 U,
+16 routing V, no new missing outputs. The two KiCad DRC gainers and three
connection regressions are investigated below; the DRC increase is retained in
the trade-off. Nominal and its smoothing combination remain on hold for DRC
and missing-output trade-offs. Images and standalone smoothing do not currently
improve PCBench connections. A small regression count is not an automatic veto.

Latest status (2026-09-09 UTC): all experiments and focused repeats are
complete. Three fixes are separately committed on main6886640. Final
workspace gate: 2,565 passed, zero failures or warnings. Final matched
corpus result: -935 unrouted / +14 routing violations; KiCad -882 / +15,
Java -53 / -1. KiCad mask errors +151. Two routing-clean gains, no clean
losses. The automatic regression gate fails and is retained in the PR
report. Nominal clearance remains held; the negative standalone workbench
zero-guard run and its follow-up repeats are retained. See
[routing-quality-comparison.md](routing-quality-comparison.md),
[routing-quality-pr-results.json](routing-quality-pr-results.json), and
[the full benchmark summary](routing-quality-pr-performance.md).
All server results are backed up; the temporary server is released.

## Measurement audit (2026-09-08)

The supplied baseline export contains 845 rows: 740 PCBench/KiCad and 105 Java DRC
fixtures, **no local KiCad fixtures**. Its actual totals are 8,423 unrouted,
20,216 violations, 49,052.53 CPU seconds. PCBench contributes 5,570 unrouted,
921 violations, 36,614.29 CPU seconds; Java fixtures contribute 2,853 unrouted,
19,295 violations, 12,438.24 CPU seconds. The task's headline totals exactly match the neckdown export `rs-union`, not
main; comparisons use the actual main baseline export.

Downloaded all baseline referee reports from workbench into
`/tmp/copperroute-quality`. PCBench routing errors break down as: clearance 256,
track_width 250, copper_edge_clearance 246, shorting_items 72, hole_clearance 48,
starved_thermal 38, via_diameter 11. The suggested hole-clearance dominance does
not match these KiCad reports.

## Approach 1: investigate missing input rules (no router patch)

Compared originals, ground truth, exported DSNs and routed DRC:

- `GameTiger_GameTiger`: designer has zero routing errors, zero unconnected,
  1,698.2645 mm and 42 vias. Our board has 199 track-width errors. KiCad requires
  minimum 0.2 mm; **the DSN explicitly requests 0.1778 mm** both globally and in
  the net class. This is not evidence for an incorrect Rust width calculation.
- `mdbwerk_mdbwerk`: designer has zero routing errors, zero unconnected,
  523.3461 mm and 34 vias. Original P2 pads explicitly carry 0.8 mm clearance;
  DSN has only 0.1524 mm global/class clearance and no per-pad clearance.
  Routed DRC reports a trace 0.4045 mm from a P2 pad, violating the absent rule.
  Increasing global hole clearance cannot restore this missing pad constraint.
- `espalarm_alarm`: original has zero routing errors (four invalid-outline
  errors), zero unconnected, 1,966.4677 mm and 85 vias. Routed board has 78
  copper-edge violations. DSN supplies a rectangular boundary; further outline
  comparison is needed before attributing these to the router.

Decision: do not guess a global minimum width or pad clearance in the DSN router.
It cannot infer omitted KiCad constraints. No candidate change or performance claim.

Outline follow-up: `espalarm`'s original has sixteen Edge.Cuts lines/arcs,
including recesses. DSN reduces this to a rectangle. This confirms another
missing-input cause, not evidence for inadequate global edge clearance.

## Approach 2: reconcile maze and insertion clearance (promising; repair churn)

`AVR-Playground_hello_world` baseline: one unrouted, no KiCad errors, 108.4852 mm,
zero vias. Original: fully connected, no routing errors, 162.3482 mm, zero vias.
Read the designer's five B.Cu segments for `Net-(R1-Pad2)`: they travel around
and underneath the DIP package using 0.606 mm traces. DSN requests 0.6096 mm.

Environment-gated per-attempt/per-segment logging reproduces the same insertion
failure on every pass, although maze search finds a connection. A segment at
x=1155350 passes the rectangular U1 pin whose left edge is x=1160400. Half-width
3048 leaves 2002 units against a 2000-unit clearance. The uncompensated tree's
insertion query adds `CLEARANCE_SAFETY_MARGIN=16`, requiring 2016 units, while
maze compensation excludes that margin and locator tolerance is only 2.
Hypothesis: accepting nominal-clearance-safe shapes in that overlap query can
recover connections without sacrificing KiCad clearance. This is a deliberate
Java divergence; specified clearances remain enforced. Failing-first test uses
202- and 198-unit gaps around a pad with a 200-unit rule.

Candidate implementation changes only the uncompensated overlap query's matrix
lookup from margin-inclusive to nominal. Watched the new test fail on 202 units,
then pass with the change; all 36 search-tree tests pass. Removed temporary logging.

Local KiCad validation (`quality-local-base` vs `quality-local-nominal`, 11 boards,
10 passes, 120-second limit, one thread, four jobs): 3 boards improve, 0 regress,
25 fewer unrouted; 1 board gains DRC errors, net 2 fewer violations; CPU ratio
0.98555. `motorizedopener`: U 65→57, V 37→38; `NoWiresOnPowerLayers`: U 16→0,
V 3→0; `Tastexx`: U 1→0, V 0→0. No referee failures.
Separate KiCad reproduction (`quality-avr`): U 1→0, V 0→0; completed route
135.5826 mm, zero vias (83.51% of original length). Baseline 108.4852 mm was
incomplete and is not a fair length-efficiency comparison.

Full run `quality-nominal-full` is detached on workbench. The 740 PCBench boards
run before the 105 Java fixtures. Uses a separate workbench worktree and explicit
referee environment variables. Candidate identity `636e039-nominal` identifies an
uncommitted patch, not an invented git commit. Baseline binary rebuilt separately
at `/tmp/copperroute-quality-base`; `quality-base-sanity` checks three known boards.

`cargo test --workspace` exposed geometry/transcript parity differences, so a
`--no-fail-fast` run is collecting them all. Reference recordings remain untouched
pending corpus evidence. User explicitly confirmed that Java divergence is welcome.

Baseline referee canary completed: all three `quality-base-sanity` boards match
the saved baseline exactly in U and V, including `GameTiger`'s 199 errors and
`espalarm`'s 78. No referee failures. CPU ratio 1.0514 on this small rerun.
First full-run board gaining DRC is `APM-RPi-Shield`: all five additional errors
are copper-edge clearance, not trace/pad clearance. No aggregate conclusion yet.

## Approach 3: nominal clearance only around pins (local experiment)

The reproduced failure involves a pin. Approach 2 also changes clearance queries
against movable tracks and vias during shoving/optimization. Test a narrower
permission extension: nominal clearance for pins, historical margin for other
items. It retains all previously allowed geometries, and tests whether broader
relaxation is needed. The nominal candidate's source and binary were archived;
the ongoing workbench run is immutable and unaffected by local experimentation.

Pin-only local results (11 KiCad fixtures): `NoWiresOnPowerLayers` U 16→0/V 3→0;
`motorizedopener` U 65→60/V 37→38; `dev-board` U 0→0/V 0→1. `Tastexx` stays at
one unrouted (broad nominal recovered it). ADC reproduction: baseline and pin-only
both complete with 0 U/0 V, at 39.96 and 42.89 CPU seconds locally respectively.
The broad nominal candidate still runs into the time limit. No pin-only corpus
claim yet.

## Approach 4: stop one-unit smoothing churn (independent candidate)

A two-second native stack sample of broad-nominal ADC routing placed 1078/1079
samples in fanout's changed-area optimization, mostly smoothing and normalization.
Existing `P7T8B_OCA` instrumentation reproduced thousands of repeated one-unit
junction moves in five seconds, before the main autorouter began. Example:
`prevDist=13342 otherDist=1 tdist=1`, followed by forced translation of one unit.

`acute_add_line` first subtracts a one-unit reserve, then clamps the move to at
least one unit. With only one unit available, the clamp defeats the reserve. A
failing-first test requires no move with 1 or 1.5 units available and a one-unit
move when two units are available. It failed at available=1 on old code and passed
with a two-unit minimum and removal of the now-redundant clamp. This stops tiny
optimization steps; it does not reduce allowed trace widths or nominal clearances.

Testing independently on baseline, and also reproducing ADC with broad nominal
plus smoothing correction. Source copies and candidate binaries are archived in
`/tmp/copperroute-quality`; workbench's broad-nominal binary is unchanged.

Broad nominal plus smoothing now completes ADC locally (0 incomplete, 6 passes,
64.982 seconds routing elapsed, fast diagnostic build with LTO disabled). This
confirms the churn explanation; elapsed time from that build is not used as a
performance comparison. Broad nominal alone reproduced 100 unrouted at 300 s
locally (285.01 CPU seconds).

Smoothing-only: `cargo test --workspace --no-fail-fast` passes with no updated
reference recordings. Local KiCad fixtures: 2 improved / 1 regressed; U delta -19;
0 boards gain violations, V delta 0; CPU ratio 1.06657. `motorizedopener` U 65→61,
V 37→37; `NoWiresOnPowerLayers` U 16→0, V 3→3 (both hit the router's internal
job deadline); `Natural_Tone_Preamp` U 0→1, V 0→0 (both completed normally).
A 60-second diagnostic run of `Starling` leaves 17 unrouted with both baseline
and smoothing, so that timeout is not explained by this particular churn fix.

The independent full run `quality-smoothing-full` is queued after nominal's
final export, using 12 jobs and a separate `target-smoothing` binary. This avoids
changing the executable of a running candidate or doubling routing workers.


## Approach 5: existing strict DRC setting (diagnostic, no code change)

Local three-board `quality-shorts-strict` compares baseline and `strict_drc=true`
with 10 passes/300 seconds. Juno: U 2→3, V 36→2, CPU 35.41→33.76 s.
Dropbot: U 3→3, V 4→4, CPU 44.41→44.94 s. Saiboard 8x3:
U 0→0, V 22→22, CPU 14.07→14.27 s. Aggregate 0 improved / 1 regressed,
U +1, no DRC gainers, V -34, CPU ratio 0.9902. No default change proposed:
it costs a connection and does not explain the track/pad shorts on the other two.
Smoothing alone on these three: Juno 2/36, Dropbot 3/4, Saiboard 0/24
(U unchanged overall, one DRC gainer, V +2). CPU 160.70 vs 93.89 s (1.7116).

## Approach 6: preserve explicitly named footprint images

Original-board inspection found that Saiboard Q3 pad 2 is at (29.1315,100.518) mm
with 1.475×0.6 mm pads. Imported DSN instead produced (29.194,100.518) mm and
0.9×0.8 mm pads: geometry from the base SOT-23 image, not SOT-23::25.
Dropbot Q6's large pad was imported at x=94.167 rather than 93.167 mm, with
6×10.5 rather than 9×11 mm bounds, and no net instead of net 22. The original
explicit image contains pin `2@1`; the incorrectly selected base image contains
`@1`. This was not a lexer error.

Two independent failures in image identity explain it: library import strips the
numbered suffix and deduplicates/renumbers images without updating placements;
package lookup lets a base-name fallback overwrite an exact opposite-side match.
Failing-first tests reproduced each. Preserve the input image name during import
and prefer any exact match before existing fallback. Duplicate definitions of
the same exact name retain their old handling. Existing missing-image fallback
also remains. Diagnostic import now matches the original pad centers and restores
Dropbot's missing pad net. No routing-quality claim until KiCad scoring.


Image-preservation local scoring (`quality-shorts-images`): Saiboard U 0→0,
V 22→0, CPU 14.07→93.42 s; Dropbot U 3→1, V 4→0, CPU 44.41→49.26 s;
Juno U 2→2, V 36→36, CPU 35.41→37.94 s. Aggregate 1 U-improved / 0
U-regressed, U -2, 0 DRC gainers, V -26, CPU ratio 1.9237. Saiboard is now
clean, using 5,904.2427 mm/130 vias versus the original's 5,925.2603 mm/148 vias
(99.65% wirelength, 87.84% vias). Baseline was 5,776.1188 mm/169 vias with
22 errors, so the shorter baseline was not an acceptable route. Dropbot uses
2,967.5538 mm/56 vias against original 2,384.1711 mm/153 vias; its remaining
one unconnected prevents claiming full success.

Local 11 fixtures (`quality-local-images`): 1 U-improved / 0 U-regressed,
U -7 (250→243), no DRC gainers, V 0 (40→40), CPU ratio 1.02880
(296.85→305.40 s). All seven recovered connections are NoWiresOnPowerLayers;
both runs hit the internal job deadline, so this part is timing-sensitive.
Full `quality-images-full` is queued after smoothing with its own immutable
`target-images` binary. No router changes are combined in these comparisons.

Parity review: preserving BBD images changes the item ids for U102-17, U102-23,
and C2-1, and the tree ordering; clearance overrides themselves remain identical.
RelayModule retains exact image names and each image's outline order rather than
the base image's order. RelayModule/CPU-85 SES groups change, so tests retain
byte comparison outside placement and compare every placement field while
checking image names against the DSN. JVM recordings are unchanged, with
explicit reviewed deltas in test code and a separate four-row tree-order file.
The unrelated existing lexer normalization of unquoted Cyrillic FU1/FU2 image
names in RelayModule remains a follow-up lead.

Nominal full-run caveat: `keyboards_Djinn` is killed at the harness's 360 s hard
cap and produces no SES. Its fallback `unrouted=168` is **not** a valid improvement
over the baseline's 396: the fallback counts nets, not missing connections.
Exclude this row from numeric paired quality/CPU statistics, and report the
missing output separately as a regression. Its referee failure must never be
misrepresented as zero violations.


Further original-board checks split the 48 hole-clearance errors into 34 labeled
`pad clearance` and 14 labeled `board setup constraints hole clearance`.
`Hardware_Playground_rpi_zero` has original pad-local 0.3 mm clearances but its
DSN supplies only 0.2 mm; its five hole errors report 0.2905–0.2969 mm against
0.3 mm. `bikedar` has original NPTH pad-local 1.65 mm clearance but DSN only
0.15 mm globally; all four reported hole errors cite that missing 1.65 mm rule.
Both originals have zero routing DRC and no unconnected items.

`Pi5_PCIe` requests a 0.35/0.20 mm via for its high-speed class in the DSN;
KiCad requires diameter >=0.45 mm, explaining its eleven via-diameter errors.
A 0.075 mm annulus plus the DSN's 0.1 mm copper clearance permits hole gaps
near 0.175 mm, below KiCad's missing 0.2 mm hole constraint (four errors,
actual 0.1776–0.1949 mm). Original is fully connected/zero routing DRC with
669.99 mm/276 vias. This confirms omitted design constraints; no universal
via diameter or global clearance is guessed from these individual examples.


The final image-preservation `cargo test --workspace` run passes, including the
reviewed parity tests; no JVM reference file was overwritten. Java source at the
requested `278fe141` has the same suffix stripping (`Library.java:409`) and
other-side/base-name overwrite (`Packages.java:28`), confirming deliberate
improvement over shared behavior rather than a Rust-only port mistake.


Nominal PCBench portion is complete: 739 scored / 1 missing output, 122
U-improved / 56 U-regressed, U -271, 30 DRC gainers, V -1, CPU ratio 0.8984
on scored pairs. The initial decision was to reject this broad form because the aggregate hides
100 added unrouted on ADC, 35 on GameTiger, and a new missing-output failure.
That decision was reconsidered below against the task’s aggregate criterion. Java fixture
measurements continue to complete the record. Pin-only remains an unvalidated
narrower alternative, not a landed fix.

## Approach 7: local composition of the two reproduced fixes

Image preservation plus the smoothing reserve correction retains Saiboard 0 U/0 V
and Dropbot 1 U/0 V. CPU is 88.10 and 43.64 s versus image-only 93.42 and 49.26 s
(ratio 0.92333 for the pair); against main, 1 U-improved / 0 U-regressed, U -2,
no DRC gainers, V -26, CPU ratio 2.25274. This is only a two-board composition
pilot, not evidence that the independent corpus effects compose safely.


Combined local 11 fixtures: 2 U-improved / 1 U-regressed, U -19 (250→231),
no DRC gainers, V 0 (40→40), CPU ratio 1.09864 (296.85→326.13 s).
Same U/V totals as smoothing alone on this set; no composition corpus claim.
Queue update: prioritize image preservation after nominal; smoothing now waits
for image preservation's final export. Only waiting processes were restarted;
no routing attempt or active candidate binary was interrupted or replaced.


Reconsideration: a few severe individual regressions do not automatically veto
an aggregate improvement under the stated acceptance criterion. Nominal clearance
remains promising: -271 U/-1 V across the 739 scored PCBench pairs. Missing output
remains separately reported, not converted into a fabricated numerical win.
Since the smoothing correction already repairs the reproduced ADC regression,
run nominal + smoothing over the same 845 boards before deciding whether to land
clearance relaxation. A second isolated remote worktree builds this composition;
no active or queued binary is replaced. Prioritize this repaired candidate after
image preservation and before the standalone smoothing run.


### Completed nominal run and baseline-failure audit

Export: `quality-nominal-full-nominal-636e039.json`, 845 attempted rows. Baseline
referee reports reveal four pre-existing unscored Java cases: `issue006` and
`issue756-tomu-fpga7/8/9`. They also fail in nominal; they are **not new
regressions**. Their export rows conceal failure behind zero CPU and fallback
unrouted counts (0, 69, 69, 69). Only PCBench Djinn is newly unscored. Earlier
progress descriptions treating the Java missing outputs as new were incorrect.
Baseline therefore has 841 scored boards; nominal has 840.

| Referee | Scored pairs | U improved / regressed | ΔU | DRC gainers | ΔDRC | CPU ratio | Clean boards |
|---|---:|---:|---:|---:|---:|---:|---:|
| PCBench / KiCad | 739 | 122 / 56 | -271 | 30 | -1 | 0.89842 | 479→521 |
| Fixtures / Java DRC | 101 | 38 / 4 | -248 | 1 | +67 | 0.96775 | 22→29 |

Totals on these pairs: U 7,820→7,301 (-519), V 20,216→20,282 (+66).
KiCad removes 138 violations and adds 137; Java removes one and adds 68.
CPU: KiCad 36,270.82→32,586.62 s; Java 12,438.24→12,037.18 s.
All four baseline-unscored Java cases remain unscored, plus one new PCBench
failure. The raw export's apparent 747-U reduction includes a fabricated
228-U gain from Djinn's fallback and must not be used as a quality claim.

Repaired nominal local pilot so far: ADC main/repaired both 0 U/0 V,
CPU 35.35→45.05 s; GameTiger U 31→25, V 199→199, CPU 45.01→41.92 s.
The prior unrepaired nominal full run was 66 U on GameTiger. Djinn is pending.


Repaired nominal pilot completed on three KiCad boards: 2 U-improved / 0
U-regressed, U -12, no DRC gainers, V 0, CPU ratio 1.01167
(401.78→406.47 s). Djinn now produces an SES: U 394→388, V 0→0,
CPU 321.42→319.50 s. Both runs report internal TIMED_OUT; these six connections
are deadline-sensitive, and neither is claimed fully routed. The independent
full run remains necessary. ADC's previously reproduced 100-U regression is gone.

Nominal full KiCad type deltas: clearance +15, copper-edge +10,
hole-clearance 0, shorts -17, starved-thermal -4, track-width -3, via-diameter -2.
Solder-mask bridge reports rise by 333 but are outside the benchmark's routing
DRV metric; they are not folded into its reported V total. This is a concrete
secondary trade-off, not evidence that every aspect of DRC improved.


Image-run referee retry: ESP-Breakout's importer failed with GTK diagnostics.
Its parsed route geometry is exactly identical to the baseline (only placement
image groups differ, which the bench importer ignores). Preserved initial logs
in workbench `/tmp/quality-esp-initial-referee-failure`. A first manual scoring
attempt refused to run because the CLI environment override was missing; reran
with the complete bench environment and a fresh Xvfb display. Same SES scores
0 U/0 V, matching baseline. No re-routing or route selection occurred.

Djinn follow-up, no router change: KiCad scores a synthetic footprint with two
disjoint same-number pads as one missing connection, exactly like two different
pad numbers. This refutes blindly treating `@`-suffixed DSN pads as electrically
connected merely because their base pad number matches. Do not use that shortcut
to reconcile Rust's larger internal incomplete count with KiCad's result.


### Repaired nominal workspace inventory and image progress

`cargo test --workspace --no-fail-fast` on the isolated nominal + smoothing
worktree completed: 2,528 passed, 28 failed across 13 targets. These failures
remain under review; the candidate does **not** yet pass the workspace gate.
The failing suites include geometry transcripts, forced insertion/via behavior,
and fixed route output expectations. No JVM recordings have been overwritten.
A representative changed probe is `twoSegments shape=0`: nominal clearance
permits the check at recursion depth zero, where the margin required a shove.
This needs an explicit expectation and a separate genuinely obstructed probe
to retain recursion-limit coverage, rather than dropping that coverage.

Image preservation has 299 scored PCBench boards: 2 U-improved / 2 U-regressed,
net U 0, two DRC gainers, net V +6, CPU ratio 0.9701. These are partial results,
not an acceptance decision. The largest DRC regression is serial_gw_ATMEGA328P
(+8): its candidate report contains seven clearance and seven hole-clearance
errors, all citing the pad's 0.3000 mm clearance. Reported copper gaps range
from about 0.2145 to 0.2346 mm, with corresponding drill gaps 0.2645–0.2846 mm.
The report alone does not establish whether image geometry or omitted input
constraints caused those errors; inspect the original and DSN before attributing
the regression or attempting a repair.


### Nominal test review: seven failures resolved

Changes are in the isolated `copperroute-quality-combined` worktree and do not
alter the immutable benchmark binary. Targeted tests pass for these cases:

- Three board-ext failures: explicitly pin the one changed JVM row, including
  its cleared failing-obstacle field. The original zero-recursion check now
  succeeds; increasing all relevant nominal rules by eight units restores
  failure at zero and success at one recursion level. Both cases remain tested.
- Micro-neckdown: the near pair accepts widths 60 and 69 rather than falling
  through to 50. Override only those two recorded width fields; preserve
  insertion-order and other geometry checks and the unchanged JVM recording.
- Core wrapper: compare SES bytes and final statistics with a direct call to
  the underlying router on a cloned prepared board. JVM route bytes are no
  longer an appropriate oracle for wrapper transparency. Both routes agree.
- CLI JSON: nominal routing has 14 trace objects instead of 13, still two vias.
  Keep the independently generated JSON-versus-SES trace and via count checks.
- MCP settings: Issue433 still needs multiple autorouting passes, but merely
  replacing Issue733 did not distinguish a one-pass limit: the later optimizer
  erased that difference. Disabling the optimizer in both compared runs makes
  the pass-limit test pass and directly tests the intended setting.

The 28-failure inventory now has seven individually resolved cases; the full
workspace gate has not been rerun or claimed green for this candidate.

Image preservation at 393 scored PCBench boards: 3 U-improved / 4 U-regressed,
U +83, four DRC gainers, V +11, CPU ratio 0.9675. The two largest U regressions
are amalthea (40→108 U, 0 V, CPU 304.00→303.85 s) and ReSDMAC
(133→149 U, 0 V, CPU 300.61→301.20 s). Both candidates report internal
TIMED_OUT; the export's `timed_out: false` does not capture that internal state.
These regressions count despite being deadline-sensitive.

Serial-gw original inspection confirms zero original routing DRC violations,
672.9181 mm / 25 vias, and a 0.3 mm clearance on the implicated VIA_MATRIX pads.
The DSN contains global 200 um clearance and 50 um smd_smd clearance, without
that 300 um per-pad rule. The missing constraint explains the reported rule
mismatch; the candidate's additional violations still count in acceptance.


### Further nominal review and deadline pilot

Four tightening tests now pass. Preserve all old recordings and apply a sparse
reviewed table containing mode, row number, exact old row, and exact new row.
There are 28 changed rows total: two fixed 45-degree rows, six random 90-degree
rows, nine random 45-degree rows, and eleven random any-angle rows. Fixed rows
tighten 16 units closer; random rows change eligible corner removals and their
resulting polylines. The whole tightening target passes 22 tests.

The board-history target passes all 18 tests using a separate nominal recording;
the previous recording is retained. The changed routing pool has different
scores and ranks (B3 199.99211→399.98834, B4 359.9864→559.98267), so the previous
fixed rank transcript no longer describes that pool.

The four-layer via-check test explicitly allows its one changed probe (119):
the 93-unit-radius via at (-499,653) has approximately 212.63 units of Euclidean
clearance to the square pad centered at (-1000,400). Its nominal 200-unit check
now succeeds; all other 199 recorded answers remain unchanged. This brings
individually resolved failures to 13 of the original 28-test inventory.

The mutating random-via test reveals additional cases after its earlier
assertions are resolved. A baseline reproduction of block 0, row 101 leaves
the via at (-319,363); nominal moves it to (-475,519) before the enclosing pad
operation fails at recursion depth zero. The legacy API permits partial work
on failure. Two block-1 pin-obstacle refusals become successes without changing
the board. Block-2 differences, including a shortened existing trace in row 67,
remain under review; no broad parity acceptance or green workspace is claimed.
All temporary instrumentation in the primary worktree has been removed.

Image full run at 514 PCBench boards: 7 U-improved / 5 U-regressed, U +55,
six DRC gainers, V +14, CPU ratio 0.9743.

Started `quality-images-deadline-pilot` locally on Amalthea and ReSDMAC, comparing
main, images, and images+smoothing at 10 passes / 300 seconds / one thread /
three jobs, with the same original and stripped KiCad files and project rules
as workbench. Completed Amalthea cells reproduce the regression: main 44 U/0 V,
184.14 CPU seconds; images 108 U/0 V, 298.21 CPU seconds. Main ReSDMAC has
117 U/0 V, 297.32 CPU seconds. Other cells are pending. A three-second Amalthea
image profile captured 1,783/1,848 active samples in fanout and 213 in its
optimization call (about 12%); it does not reproduce ADC's overwhelmingly
smoothing-dominated profile. Do not assume the smoothing patch repairs it.


### Deadline composition rejected locally; nominal test review continues

The six-cell deadline pilot completed with valid KiCad scores.

| Candidate | U improved / regressed | ΔU | DRC gainers | ΔV | CPU ratio vs local main |
|---|---:|---:|---:|---:|---:|
| Images | 1 / 1 | +62 | 0 | 0 | 1.24197 |
| Images + smoothing | 1 / 1 | +62 | 0 | 0 | 1.24955 |

Main: Amalthea 44 U/0 V/184.14 CPU s, ReSDMAC 117 U/0 V/297.32 CPU s.
Images: 108/0/298.21 and 115/0/299.75 respectively. Images+smoothing:
108/0/301.53 and 115/0/300.08. Smoothing changes neither quality result and
increases CPU by about 0.61% versus images. This refutes its proposed repair
of these two regressions. Built an immutable remote images+smoothing binary
in `target-images-smoothing` (release build succeeded in 2m36s), but do not
queue another full composition based on this negative pilot. The existing
queue remains images → nominal+smoothing → standalone smoothing. A proposed
queue change was reconsidered before any process was canceled or replaced.
The remote source was restored after building; all three production-file
SHA-256 hashes match the primary footprint worktree.

Image full run at 692 scored PCBench pairs: 13 U-improved / 7 U-regressed,
U +31, six DRC gainers, V +8, CPU ratio 0.9862. There is now a NEW unscored
Djinn missing-output failure, excluded from those pairs and counted separately.
As with nominal, its fallback export count must not masquerade as a win.

The nominal forced-via target now passes all 26 tests. Reviewing all five
mutating blocks together exposes 21 changed cases among 600: eight former
refusals succeed, no successes become refusals, and the remaining differences
are shove geometry, partial work on failure, or generated ids. A separate
21-row table pins old and new result/hash/item-count/max-id tuples; all original
recordings remain unchanged.

The nominal board-ext target passes all 23 tests. An exact sparse table accounts
for 18 polyline rows, 136 tail-tightening rows and 43 neckdown rows. All of these
changes are board geometry or generated-id metadata; **no insertion endpoint
row changes**. Existing stale-obstacle counts (50, 134 and 2), closing-line
allowances, and the 100-wide failure / 60-wide success neckdown assertions remain
active and pass. This brings individually resolved failures to 17 of the
original 28-test inventory.

Prepared diagnostic outputs for the remaining via-optimizer review, then removed
the temporary dump tests. Nominal's routed RPi prefix has eight vias, all with
two trace contacts, whereas the previous prefix had six including two with one
contact. Merely changing expected counts would lose one-contact optimizer
coverage. Preserve that branch with an explicit fixture while updating the
integration expectations for the better-connected nominal prefix. This review
is not yet complete, and the nominal workspace gate is still pending.


### Nominal + smoothing clears the workspace and local KiCad gates

All 28 originally failing tests have been reviewed and resolved. The final
`cargo test --workspace --no-fail-fast` run completed successfully: **2,556
passed, zero failed**, log `/tmp/copperroute-quality/workspace-nominal-reviewed.log`.
No JVM recordings were overwritten.

The via-optimizer integration recording now describes the actual eight
two-contact vias. An explicit bent fanout fixture retains the one-contact
behavior in both trace orientations: the non-mutating calculation preserves
the board, both optimizer entry points reach the expected endpoint, and adding
a second trace contact makes plane/fanout optimization refuse without mutation.
Both via-optimizer test targets pass (8 and 7 tests).

The remaining optimization and router references are separately named nominal
recordings. They explicitly pin connections 3 and 8 changing FAILED→ROUTED,
with the prior JVM states still asserted as the original regression. The
long-route insertion comparison overrides one reviewed polyline row; the
unchanged original row must match before applying that exception. The three
remaining targets pass (11 insertion, 12 changed-area, 2 reference tests; their
pre-existing ignored tests remain ignored).

`quality-local-nominal-smoothing` completed all 11 KiCad fixtures at 10 passes,
120 seconds, one thread and four jobs. Every baseline/candidate referee status
is `ok`. Relative to the matching local main run:

| Referee | Boards | U improved / regressed | ΔU | DRC gainers | ΔV | CPU ratio | Clean boards |
|---|---:|---:|---:|---:|---:|---:|---:|
| Local KiCad | 11 | 3 / 0 | -24 | 1 | -2 | 0.95627 | 6→8 |

Totals: U 250→226, V 40→38, CPU 296.85→283.87 s. MotorizedOpener U
65→58 and V 37→38; NoWiresOnPowerLayers U 16→0 and V 3→0; Tastexx U
1→0 with zero V. The motorized board is one connection worse than nominal-only
locally; that cost is retained rather than hidden by the aggregate comparison.

Footprint experiment files are archived reversibly in
`/tmp/copperroute-quality/images-reviewed.patch`, `images-reviewed-files.zip`,
and `images-reviewed-files.json` (file hashes and baseline commit), while the
primary worktree still holds that experiment pending the final export. The
original main checkout retains only its two pre-existing untracked candidate
files; no tracked files there were changed.

The remote image run has completed all 740 KiCad attempts (739 scored):
14 U-improved / 10 U-regressed, U +71, six DRC gainers, V -25, CPU ratio
1.0005, plus one new Djinn missing-output failure. At 90 scored Java fixtures
its Java delta is U -6/V 0; the run is still finishing. Repaired nominal is
queued next, followed by standalone smoothing. No production change has been
committed before that full-corpus validation.


### Footprint final result and repaired nominal progress

`quality-images-full` exported all 845 attempts. Paired scored results exclude
four Java cases already missing output in main and one newly missing PCBench
Djinn output:

| Referee | Paired boards | U improved / regressed | ΔU | DRC gainers | ΔV | CPU ratio |
|---|---:|---:|---:|---:|---:|---:|
| KiCad / PCBench | 739 | 14 / 10 | +71 | 6 | -25 | 1.0005 |
| Java fixtures | 101 | 8 / 6 | -73 | 1 | +1 | 1.0057 |

Reject this footprint candidate for landing: its aggregate two-connection gain
comes from Java fixtures while KiCad loses 71 connections, with a new missing
output on Djinn. Its 24 fewer aggregate violations do not erase those costs.
The corrected footprint identity remains a demonstrated root cause, but this
implementation needs a better routing outcome. Export and final referee audit
are preserved in `/tmp/copperroute-quality/quality-images-full-images-636e039.json`
and `images-full-status.txt`.

After archival, the primary `routing-quality` worktree now contains the reviewed
nominal-plus-smoothing candidate, transferred byte-identically from the helper
worktree after checking matching base commits and file hashes. The rejected
footprint experiment is no longer the primary worktree's production change.
No changes are committed yet.

`quality-nominal-smoothing-full` is running. At 118 scored PCBench boards:
16 U improved / 10 regressed, ΔU -29, six DRC gainers, ΔV +23,
CPU ratio 0.7701, no newly unscored boards so far. This is an early partial
result, not acceptance evidence: fewer connections are currently accompanied
by more violations. Standalone smoothing remains queued afterward.

The additional nominal-only KiCad audit found 333 more solder-mask bridge
reports (13,748→14,081), excluded from the benchmark's routing-violation total.
Forty-two boards gained reports, with 489 gross additions and 156 removals.
This secondary trade must be reported separately from benchmark clean-pass
counts; a clean pass does not imply zero reports across every KiCad category.
Audit artifact: `/tmp/copperroute-quality/nominal-drc-audit.txt`.


The final primary-worktree `cargo test --workspace` command exited 0, with
2,556 passed and zero failed. Log:
`/tmp/copperroute-quality/workspace-primary-final.log`. This verifies the
transferred candidate in the actual worktree intended for committing.

A subsequent full-run snapshot reached 197 scored PCBench boards: 33 U improved /
14 regressed, ΔU -49, ten DRC gainers, ΔV +46, CPU ratio 0.8283, no newly
unscored outputs. GameTiger improved by six connections, consistent with the
repair pilot. The growing partial DRC increase remains a concern, not a
successful acceptance result. Full measurement is still running.


### Isolating the smoothing repair in the active full run

Read-only matched comparison of repaired nominal against nominal-only reached
229 scored PCBench pairs: two U-improved / zero U-regressed, ΔU -141,
zero DRC gainers, ΔV 0, CPU ratio 0.928821. Thus the repair has not added
benchmark DRC violations on this subset; the current positive DRC delta against
main is shared with nominal-only. This remains a partial comparison, not a full
acceptance result. Script: `/tmp/copperroute-quality/compare-repair.py` (remote
`/tmp/quality-compare-repair.py`). At the preceding 221-board snapshot versus
main: 37 U-improved / 15 regressed, ΔU -58, ten DRC gainers, ΔV +38,
CPU ratio 0.8131, no newly unscored outputs. The benchmark process was verified
live as PID 3974404, not inferred from a stale output file.


At 275 scored PCBench boards versus main, repaired nominal has 45 U-improved /
17 regressed, ΔU -198, eleven DRC gainers, ΔV +20, CPU ratio 0.8230,
no newly unscored outputs. The subsequently measured repaired-versus-nominal
subset reached 289 pairs: four U-improved / zero regressed, ΔU -146,
zero DRC gainers, ΔV 0, CPU ratio 0.934146. Both are partial results.
The active bench process PID 3974404 was confirmed live at 18:12 elapsed;
standalone smoothing's waiting process PID 3905421 was also confirmed live.
The source remains unchanged after the successful primary-worktree test gate.


### Verify error severity in the supplementary KiCad audit

`compare-referee-types.py` reads each successful `referee.json` and its
`violations_by_type` map, which the referee constructs from severity=error
records only. This independently confirms nominal-only's 739-pair error
deltas: clearance +15, copper-edge +10, shorts -17, thermal -4, width -3,
via diameter -2; routing total -1. Solder-mask bridge errors increase 333
on 42 boards, so all-error total increases 332. The earlier mask audit was
not merely counting warnings.

The repaired candidate's 302-pair partial audit has routing errors +11:
clearance +8, copper-edge +9, hole +3, shorts -4, thermal +1, width -4,
via diameter -2. Mask bridge errors are +224 on 18 boards, producing an
all-error delta of +235. These supplementary counts remain part of the
tradeoff assessment despite exclusion from the benchmark routing score.
Script preserved at `/tmp/copperroute-quality/compare-referee-types.py` and
remote `/tmp/quality-compare-referee-types.py`. No scoring rules were changed.


### Standalone smoothing passes its independent workspace gate

Prepared detached worktree `../copperroute-quality-smoothing` at main 636e039,
containing only the `tightener_45.rs` repair and its original failing-first test.
`cargo test --workspace --no-fail-fast --target-dir
/tmp/copperroute-quality/target-smoothing-tests` exited 0: **2,555 passed,
zero failed**, with no parity expectation updates. Log:
`/tmp/copperroute-quality/workspace-smoothing-only.log`. Its queued corpus run
is still required before committing this change separately. A post-run tally
initially asserted the combined candidate's 2,556 count, which was an incorrect
expectation for this different test inventory; the Cargo command itself passed.

The combined full run remains live, at 458 scored PCBench boards: 74 U improved /
35 regressed, ΔU -309, 21 DRC gainers, ΔV +55, CPU ratio 0.8181,
no newly unscored outputs. This is still a partial result, and the increased
DRC cost is not being dismissed based on connection gains.


At 462 matched PCBench boards versus nominal-only, the repair has eight
U-improved / four regressed, ΔU -136, zero DRC gainers, ΔV -8,
CPU ratio 0.956339. It is no longer regression-free on connections: e.g.
decelerator U 456→470 while V 11→6. Azalea U 77→69 and V 57→56;
esp32-ethernet stays fully connected with V 3→1. These matched changes are
separate from the candidate-versus-main acceptance comparison.


### Djinn missing-output regression survives the combined full run

Observed repaired Djinn router PID 4013605 live at 2:52, 3:56, 5:02,
and 5:57 elapsed, consuming approximately one full CPU throughout. No SES
was present at 5:57. The subsequent referee record is terminal
`referee_failed`, reason `out.ses missing (candidate produced no output)`.
Thus nominal+smoothing does **not** fix this missing-output regression on
workbench, despite the successful local repaired pilot. Do not present the
local result as a proven corpus repair. Exclude Djinn from scored paired
metrics and report the new output failure separately, as for nominal-only.
The full corpus run continues unchanged; the unresolved post-deadline work
requires further investigation before accepting this candidate.


### Djinn deadline profile identifies repeated census connectivity work

Ran the immutable nominal+smoothing binary locally on the original Djinn DSN
at ten passes, a five-minute timeout, and one thread. Sampled router PID 90306
three times with macOS `sample`, without source instrumentation. Early sample:
902/926 active stack samples are under BoardStatistics → unconnected_set →
connected_set; the middle sample likewise spends most sampled time collecting
connectivity statistics. Crucially, the sample at 5:06 elapsed (after the
five-minute deadline) still shows 957/1407 active samples in that connected-set
path beneath fanout statistics. The local process eventually exited 0 and
wrote a SES, unlike the workbench hard-cap outcome.

Source inspection shows `fanout_census` independently invokes `unconnected_set`
for every SMD pin/net. Each traversal repeats geometric `normal_contacts`
queries; outer fanout and changed-area loops already poll their stop checks.
Next hypothesis: cache contact queries within one immutable census to eliminate
repeated geometry work, preserving directed reachability and all existing
connectivity answers. Do not replace this with a blind deadline guard or merge
same-number pads. No implementation change has been made yet.

Artifacts: `/tmp/copperroute-quality/djinn-deadline-{early,middle,late}.sample.txt`,
`djinn-deadline-diagnostic.{json,ses,stdout,stderr}`. The profiler observations
establish a local post-deadline bottleneck, not proof that every workbench
missing-output case has the same stack. A fix still needs a failing-first
regression, exact result checks, and real full-corpus validation.


### Census contact-cache experiment: failing-first regression and implementation

Created detached worktree `../copperroute-quality-census` at main 636e039.
This experiment contains only the census change, allowing its independent
quality effect to be measured. The helper initially used uncached normal-contact
queries and reproduced `unconnected_set(...).is_empty()` with iterative directed
reachability. A regression queries every item in the existing multi-net SMD
fixture using its actual nets, net 0, negative net, and a non-member positive net,
and checks every answer against the current Board implementation. Repeating the
queries in reverse must perform no additional geometric contact searches.

Observed the intended red failure: all connectivity answers matched, but the
contact-query count rose 36→72 on the second pass. Log:
`/tmp/copperroute-quality/census-red.log`. Added a contact cache scoped to the
immutable census; the same test now passes (`census-green.log`). The census
computes connectivity predicates before its mutable escape/DRC queries, so the
cache cannot survive a board mutation. No geometric guards or scoring rules are
changed, and no undirected-component assumption is made.

Full workspace tests are running in `/tmp/copperroute-quality/workspace-census.log`
and a release build in `build-census.log`. The new candidate has **not** yet been
measured on a board or queued on workbench. First measure real Djinn behavior;
then validate any worthwhile version across the corpus before landing.

The existing nominal+smoothing run remains unchanged: at 721 scored PCBench
boards, 117 U-improved / 50 regressed, ΔU -411, thirty DRC gainers,
ΔV +3, CPU ratio 0.8626, plus the separately reported new Djinn output failure.
Java fixtures are still pending. This remains partial, not acceptance evidence.


The standalone census release build succeeded in 1m24s. Started
`quality-census-djinn-pilot` in the primary benchmark directory, comparing
immutable main and census binaries on Djinn at ten passes, timeout 300,
one thread and two jobs, with the benchmark KiCad referee. Candidate config
`/tmp/copperroute-quality/candidates.local.toml`, run log
`/tmp/copperroute-quality/bench-census-djinn-pilot.log`. Results pending;
workspace tests also still running. This census experiment is not included
in the active remote nominal+smoothing run.


### Census cache: workspace success and KiCad-scored Djinn improvement

Standalone census passed `cargo test --workspace --no-fail-fast`: **2,555
passed, zero failed, zero warnings**. No parity expectations were updated.
`quality-census-djinn-pilot` completed both referee scores successfully:

| Referee | Boards | U improved / regressed | ΔU | DRC gainers | ΔV | CPU ratio |
|---|---:|---:|---:|---:|---:|---:|
| KiCad, Djinn pilot | 1 | 1 / 0 | -161 | 0 | 0 | 0.92914 |

Main U 394, V 0, CPU 318.93 s, peak RSS 63.1 MB; census U 233, V 0,
CPU 296.33 s, RSS 152.1 MB. Both produced output and internally reported
TIMED_OUT (the benchmark wrapper's timed_out=false means no external kill).
Output wall times 324.04→302.57 s are recorded only for deadline behavior;
CPU is the performance comparison. Vias 139→90; wirelength
2,098.4501→4,399.9738 mm as more connections are routed, versus original
board ratios 0.2576→0.5401. This is a real connection improvement with a
material memory cost, still only a one-board result.

Candidate sample `djinn-census.sample.txt` predominantly shows actual fanout
connectivity work rather than the original statistics bottleneck. Started
11-fixture `quality-local-census` (log `bench-local-census.log`). Copied the
single changed source file into isolated remote worktree
`/home/em/copperroute-quality-census` at 636e039 and started a detached
CARGO_BUILD_JOBS=2 release build, log `/tmp/quality-build-census.log`.
Full-corpus census run is not launched yet; preserve the existing run queue.

The nominal+smoothing PCBench subset is now complete: 739 scored pairs,
124 U improved / 52 regressed, ΔU -415, thirty DRC gainers, ΔV -7,
CPU ratio 0.8778, plus the new missing Djinn output. At 24 scored Java pairs,
ΔU -58/V 0; the Java subset is still finishing.


### Census local fixtures pass; full run queued

`quality-local-census` completed all eleven fixtures, with every referee status
`ok`: one U-improved / zero regressed, ΔU -13, zero DRC gainers, ΔV 0,
CPU ratio 1.039582. Totals U 250→237, V 40→40, CPU 296.85→308.60 s,
benchmark clean passes 6→6. Only NoWiresOnPowerLayers changed quality,
U 16→3 with V 3 unchanged; its longer time-limited routing work outweighs
CPU reductions on the other ten fixtures. Do not claim a general CPU speedup
from the Djinn pilot alone.

Remote census build finished successfully in 2m54s. Immutable binary SHA256:
`3e1cf914a34d15ed07de78f083e36ecd25ef14e259f4ba3442c1628ccf41cb57`.
Queued `quality-census-full` after the final standalone-smoothing export, using
all 845 boards, ten passes, timeout 300, one thread, twelve jobs, and explicit
Java and KiCad referee environment variables. Script/config:
`/tmp/quality-run-census.sh`, `/tmp/quality-census-candidates.toml`; log
`/tmp/quality-census-full.log`. The existing queue order is preserved.

Copied the census source into the local nominal+smoothing helper worktree
(after verifying its previous statistics file matched main). The primary
routing-quality worktree remains nominal+smoothing without census. Combined
workspace tests are running in `workspace-combined-census.log`; combined
release build succeeded into `target-combined-census-release`. Started
`quality-combined-census-djinn-pilot`, comparing nominal+smoothing with and
without census at the same ten-pass/300-second/one-thread settings, two jobs.
Run log `bench-combined-census-djinn-pilot.log`. This composition has not been
queued for a full corpus yet; the standalone census is being measured first.


At 806 attempted nominal+smoothing cells, a second newly unscored output
appeared: Java fixture `issue070-autorouter_fq101_pcb_2022-05-13`, referee reason
`out.ses missing (candidate produced no output)`. This is additional to Djinn
and is not one of the four baseline failures. Exclude it from matched scored
metrics and report its output regression explicitly. Current Java subset:
64 scored pairs, 24 U-improved / one regressed, ΔU -150, two DRC gainers,
ΔV +68, CPU ratio 0.9253. PCBench totals remain 739 pairs, U -415/V -7.
The full run is still live; these Java numbers are partial.


### Combined census gates and independent issue070 diagnostic

Nominal+smoothing+census passed the full workspace: **2,557 tests, zero
failures or warnings**, log `workspace-combined-census.log`.
`quality-combined-census-djinn-pilot` completed both KiCad referees:
nominal+smoothing U 388/V 0/CPU 323.99 s/RSS 61.0 MB; adding census
U 216/V 1/CPU 300.79 s/RSS 146.3 MB. One U-improved, zero regressed,
ΔU -172, one DRC gainer, ΔV +1, CPU ratio approximately 0.928393.
Vias 139→96; wirelength 2,205.1063→4,634.7047 mm as connectivity increases.
The new error is track Net-(D36-Pad2) versus C38 pad 1 SK6812_PWR on B.Cu:
actual clearance 0.1480 mm versus required 0.1500 mm. Do not describe this
combination as DRC-neutral. Started `quality-local-combined-census`, all eleven
local fixtures, ten passes, timeout 120, one thread, four jobs.

The issue070 workbench failure was an external kill at 360.003 s (exit -9),
whereas main produced U 47/V 49 at CPU 299.16 s. Verified local/remote DSN
SHA256 both `363c687e7bba3fe8e737e888e28b5791ddcaf31b1dc7d28c3888426c99193bc6`.
A local nominal+smoothing diagnostic at the same settings exited 0 and wrote
output at 300.31 s, CPU 297.68 s, internally TIMED_OUT. Thus the missing
output was not reproduced locally. Its early sample (`issue070-early.sample.txt`)
shows normalization during insertion rather than Djinn's census bottleneck.
The attempted late sample found the process already exited; there is no local
post-deadline profile proving the workbench cause. Source inspection finds
insertion passed `&|| false`, but enabling cancellation blindly could interrupt
mutations and trigger the existing error/panic path. No patch was made.
Artifacts: `issue070-nominal-smoothing-diagnostic.{json,ses,stdout,stderr}`.


At 842 attempts, nominal+smoothing has all 839 currently scored pairs:
739 PCBench and 100 Java. Java: 37 U-improved / four regressed, ΔU -263,
two DRC gainers, ΔV +68, CPU ratio 0.9678. PCBench remains ΔU -415,
ΔV -7. Aggregate scored ΔU -678 / ΔV +61 excludes two new missing outputs
(Djinn and issue070), not counted as gains. Three Java cases already missing
output in main are still finishing; the final export is pending.

Verified the remote combined helper's old statistics source was exactly main
(SHA256 `2cbadedb2cfc432cc9ae7e8e71ee903958dc7bbb4a5e6298a70eddec276d4cfc`)
and HEAD 636e039 before copying the tested census source into it. Started a
detached release build with two Cargo jobs into the separate `target-census`
directory, log `/tmp/quality-build-combined-census.log`. The active
nominal+smoothing executable in its original target directory is unchanged.
This prepares the composition for validation without altering the running
benchmark. Combined local fixtures still pending; no combined full run queued.


### Original-board comparison explains the new Djinn clearance error

Matched C38 in the original `raw.kicad_pcb`: placement (78.581580,120.253630)
mm, rotation 170°, pad 1 local (-0.9375,0), B.Cu roundrect size 0.975×1.4 mm,
corner ratio 0.25 (true radius 0.24375 mm). The DSN uses image
`reversible-kicad-footprints:C-0805_2012Metric::4` and padstack
`RoundRect[B]Pad_975.000000x1400.000000_244.678000_um_0.000000_0`, a polygon.
The reported vertical track is x=80.2794 mm, y=117.5463..121.0036 mm,
width 0.127 mm.

Direct support-boundary calculation after the 170° transform: pad center
x=79.504837268449 mm; DSN polygon right edge x=80.065071930698 mm;
true rounded-rectangle right edge x=80.067861139306 mm. Thus clearance
is **150.828069 µm to the DSN polygon**, but **148.038861 µm to the actual
pad**, agreeing with KiCad's 0.1480 mm against the 0.1500 mm requirement.
The polygon understates this pad extent by 2.789209 µm. This identifies a
lossy rounded-pad DSN approximation for this particular new violation;
it does not prove the cause of all baseline clearance violations. No geometry
or guard patch was made. Preserve this example for future exporter/importer
geometry work rather than falsely treating the nominal path as KiCad-legal.


### Nominal + smoothing full export complete: held pending output-failure repair

`quality-nominal-smoothing-full` exported all 845 attempts. Downloaded JSON:
`/tmp/copperroute-quality/quality-nominal-smoothing-full-nominal-smoothing-636e039.json`.
Referee audit confirms four unchanged baseline Java missing-output cases and
two new failures (Djinn, issue070). The 839 scored pairs yield:

| Referee | Paired boards | U improved / regressed | ΔU | DRC gainers | ΔV | CPU ratio | Benchmark clean passes |
|---|---:|---:|---:|---:|---:|---:|---:|
| KiCad / PCBench | 739 | 124 / 52 | -415 | 30 | -7 | 0.877792 | 479→521 |
| Java fixtures | 100 | 37 / 4 | -263 | 2 | +68 | 0.967825 | 22→29 |

KiCad U 5,174→4,759, V 921→914, CPU 36,270.82→31,838.24 s.
Java U 2,599→2,336, V 19,246→19,314, CPU 12,139.08→11,748.50 s.
Aggregate scored U -678/V +61; failed outputs are separately reported,
not treated as zero-valued successful routing. Supplementary KiCad mask-bridge
errors 13,748→14,057 (+309), on 43 gaining boards; all-error delta +302.
Hold this version uncommitted pending the output-failure repair and assessment
of the remaining DRC trade. Its scored improvements do not erase those failures.

Standalone smoothing has now started its full run (bench PID 4042123).
Census-only remains queued afterward (waiting script PID 4039265).

### Combined census local fixtures and full queue

`quality-local-combined-census` completed eleven successful KiCad referee cells:
three U-improved / zero regressed, ΔU -24, one DRC gainer, ΔV -2,
CPU ratio 0.951996; totals U 250→226, V 40→38, CPU 296.85→282.60 s,
benchmark clean passes 6→8. This matches nominal+smoothing's local quality,
while the separate Djinn pilot documents the new clearance trade.

Remote combined-census build succeeded in 2m42s, immutable binary SHA256
`fc41a9a9342a8fb7d1221cd1413adac07a7cb05afc34023b8d8d39b49331b7df`.
Queued `quality-combined-census-full` after the census-only final export:
845 boards, ten passes, timeout 300, one thread, twelve jobs, all referee
environment variables explicit. Candidate key `nominal-smoothing-census`,
script/config `/tmp/quality-run-combined-census.sh` and
`/tmp/quality-combined-census-candidates.toml`; log
`/tmp/quality-combined-census-full.log`. Waiting script PID 4048256 confirmed
live. Queue order: smoothing → census → nominal+smoothing+census.
No further production modifications or commits have been made.


### Standalone smoothing partial regression on CoreOne

At 183 scored PCBench boards, standalone smoothing has two U-improved /
one regressed, ΔU +31, zero DRC gainers, ΔV 0, CPU ratio 0.9765,
no newly unscored outputs. CoreOne is the substantial regression: main
U 2/V 1/CPU 301.15 s versus smoothing U 37/V 1/CPU 301.91 s; both
internally TIMED_OUT and both produced scored output. The completed
nominal+smoothing run on the same board is U 1/V 1/CPU 302.53 s,
also TIMED_OUT. Thus the standalone smoothing change cannot be assumed
quality-positive merely because it repairs nominal's ADC behavior. The
full standalone measurement continues; no candidate was changed or committed.


### User steering: investigate concentrated regressions, do not reject by count

The user explicitly clarified that a change regressing only one or two boards
should remain under consideration, with investigation of what is unusual about
those boards. Decisions should weigh aggregate quality and specific mechanisms,
not treat a small number of regressions as an automatic veto.

Downloaded CoreOne and Librecalc original/stripped boards, DSNs, projects and
original DRC reports. Both originals are fully connected with zero routing DRC.
CoreOne: 245 nets/four layers, 7,325.5444 mm/428 vias, zero all-category DRC.
Its U801 package has 128 narrow 0.23×1.5 mm pads plus ten other pad shapes.
The designer routes DVDD across F.Cu/B.Cu/In2.Cu with 18 vias, and XL_DN0
across F.Cu/B.Cu with two vias. Main's output is 7,601.9343 mm/301 vias;
smoothing's incomplete output is 6,090.1002 mm/303 vias. The shorter length
is not a quality win when 35 extra connections are missing.

Librecalc: 128 nets/four layers, 4,438.1714 mm/309 vias; zero routing DRC,
206 all-category errors. IC1 is a 0.4 mm-pitch LQFP128 with 0.2032×0.889 mm
pads, and IC2 is a TSOP66 DDR part on the other face. The designer escapes
N-0000018 across three layers using two vias, N-0000019 onto In1.Cu with
one via, and DDR_DO7/DDR_DO10 across F.Cu/B.Cu with one via each. These are
among the nets left open by the workbench smoothing result. Original geometry
analysis script/output: `smoothing-regression-artifacts/original-routing.{py,txt}`.

`quality-smoothing-regressions-pilot` completed all four local KiCad cells:
CoreOne main U 2/V 1/CPU 167.93 s/COMPLETED, smoothing U 37/V 1/CPU
298.76 s/TIMED_OUT; this reproduces the +35 CoreOne regression. Librecalc
main U 0/V 0/CPU 160.68 s/COMPLETED, smoothing U 1/V 0/CPU 165.22 s/
COMPLETED: same regression direction but smaller than workbench's +6 at the
deadline. Across these two boards: zero U-improved / two regressed, ΔU +36,
zero DRC gainers, ΔV 0, CPU ratio approximately 1.41198.

A two-second smoothing CoreOne sample spends 741/1032 active samples in
changed-area tightening during the main route. Librecalc's sample is already
inside optimizer rerouting. This is evidence of different execution stages,
not yet proof of a repeated-geometry cycle or the exact regression mechanism.
Profiles: `smoothing-regression-artifacts/{coreone,librecalc}.sample.txt`.
Started `quality-smoothing-coreone-extended` at ten passes/600 seconds/one
thread to test recovery with more time; keep the standard 300-second corpus
numbers authoritative. Extended result pending; no new fix or commit.

### CoreOne repeated empty tightening proposal: root cause and failing-first test

The 600-second smoothing diagnostic completed with U 37/V 1, 596.71 CPU s,
303 vias and 6,090.1002 mm: exactly the same scored geometry/quality as the
300-second run. Additional time did not recover the regression.

A 180-second OCA ledger probe recorded 269,837 successful pull-tight calls on
trace 178007; the same changed region repeated 269,835 times. A bounded
geometry probe then captured twelve identical transitions: the existing tiny
closed triangle proposed `Polyline { lines: [] }`, but `change_trace` left
its original five lines unchanged. `pull_tight_with_engine` nevertheless
returned true. The triangle's corners are (1387465,-883121),
(1387465,-883137), (1387449,-883137), then back to the first point.
All temporary geometry instrumentation was removed from source after building
its separate diagnostic binary.

Added `collapsed_coreone_loop_does_not_report_an_unapplied_change` in the
smoothing helper. The translated triangle must first arise through
`change_trace`, since direct insertion correctly refuses an unfixed closed
trace. Initial fixture-construction failures were corrected; the final red
run proved an empty proposal and unchanged board geometry, then failed on the
incorrect true return (`coreone-noop-red.log`). The narrow candidate returns
false for an empty proposal before engine invalidation/change_trace. It does
not delete copper or change clearance permissions. The full tightening target
passes all 23 tests (`coreone-noop-green.log`). Release build pending into a
separate immutable target; board and corpus validation remain required.

Latest smoothing full snapshot: 504 scored PCBench pairs, eight U-improved /
seven regressed, ΔU +31, one DRC gainer, ΔV +5, CPU ratio 0.9876. The new
DRC concentration is azalea (+12); earlier reductions total seven. No new
missing outputs at this snapshot. This is partial, not an acceptance result.

The smoothing+empty-proposal-guard release build succeeded. Started local
`quality-smoothing-noop-regressions` (CoreOne and Librecalc, 300-second limit,
one thread, two jobs), and `quality-local-smoothing-noop` (eleven KiCad
fixtures, 120 seconds, four jobs), both against immutable
`/tmp/copperroute-quality/target-smoothing-noop/release/copperroute`.
Workspace tests also running in the smoothing helper; no commit yet.

At 576 attempts, the unmodified smoothing full run has a newly missing Djinn
output. Its 575 scored PCBench pairs: eleven U-improved / eleven regressed,
ΔU +40, one DRC gainer, ΔV +5, CPU ratio 0.9892. Djinn is excluded from those
pairs. Census-only and triple candidates remain queued, unchanged.

### Empty-proposal repair recovers CoreOne locally; full validation queued

`quality-smoothing-noop-regressions` completed with both KiCad referees scored:
CoreOne U 2/V 1/CPU 185.72 s/COMPLETED, 301 vias, 7,638.932 mm;
Librecalc U 1/V 0/CPU 175.40 s/COMPLETED, 233 vias, 4,332.8984 mm.
Against smoothing-only: one U-improved / zero regressed, ΔU -35, zero
DRC gainers, ΔV 0, CPU ratio 0.778309. Against main: zero U-improved /
one regressed (Librecalc), ΔU +1, zero DRC gainers, ΔV 0, CPU 1.098932.
Local CPU was shared with the fixture and workspace runs; full workbench
measurement remains authoritative. A sample of fixed CoreOne at approximately
2m30s is in optimizer rerouting, rather than the repeated empty-proposal loop
(`smoothing-regression-artifacts/coreone-noop.sample.txt`).

`quality-local-smoothing-noop` completed eleven scored KiCad cells:
U 234/V 40/CPU 321.06 s; versus main, two U-improved / one regressed,
ΔU -16, zero DRC gainers, ΔV 0, CPU ratio 1.081556. NaturalTone remains
0→1 U. Do not claim the repair resolves all standalone smoothing tradeoffs.
Workspace suite completed: 2,556 passed, zero failures, zero warnings;
log `workspace-smoothing-noop.log`.

Built the same three changed files in a new detached remote worktree
`~/copperroute-quality-smoothing-noop`; local/remote source hashes match.
Remote release SHA256:
`e83ab53fce76e2dc47488406bbd97b4a01f854fcf6b2395237bf5ea04d58c32b`.
Queued `quality-smoothing-noop-full` after the triple candidate's final export,
using the standard 845 boards/10 passes/300 seconds/one thread/twelve jobs,
with explicit referee environment. Existing queued binaries remain unchanged.
Script/config: `/tmp/quality-run-smoothing-noop.sh`,
`/tmp/quality-smoothing-noop-candidates.toml`. No changes committed yet.

### Azalea smoothing DRC regression: omitted pad-local clearance confirmed

At 657 scored PCBench pairs smoothing still has ΔU +40/ΔV +5; its only DRC
gainer is azalea (+12). Downloaded azalea original/stripped boards, DSN,
ground truth, smoothing output and referee report. Original: zero unrouted,
zero routing DRC (159 all-category errors), 3,183.4078 mm and 176 vias on two
layers. Smoothing: U 67/V 52, 2,256.1386 mm, 67 vias, CPU 299.57 s, internally
TIMED_OUT. Main U 76/V 40; therefore this is a nine-connection improvement
with twelve additional routing violations, not a DRC-neutral improvement.

All 52 smoothing clearance reports specify **pad clearance 0.1778 mm**;
all 40 baseline clearance reports specify the same. Actual smoothing gaps
range 0.1541–0.1758 mm (28 at 0.1541), above the DSN's 0.1524 mm global/class
clearance. Original `gsg-modules:0603` pads explicitly carry 0.1778 mm local
clearance; their DSN image has geometry and pin placement but no corresponding
rule. This is another instance of the missing-input mechanism already found
in Approach 1, not a new justification for guessing a global clearance.

Concrete check: R3 pad 1 is a square 0.8636 mm pad centered at
(111.506,105.410). Smoothing's vertical GND track at x=110.8185, width 0.2032,
has gap 0.6875 - 0.4318 - 0.1016 = **0.1541 mm**, matching KiCad and legal
under the supplied DSN but below the original pad rule. Two adjacent diagonal
segments also measure 0.1540898 mm. Original routing avoids this GND track
path; its nearest explicit front GND track is 2.1169 mm away (zones are not
included in that track-only comparison). Analysis script/output:
`azalea-artifacts/analyze.py`, `analysis.txt`. No router patch proposed from
information absent in its input, and no referee rules or corpus inputs changed.

### Karabas revisions B/G: original comparison and focused repair experiment

The next largest smoothing connection regressions after CoreOne are karabas
B (+12 U) and G (+8 U). Both main and smoothing hit their internal 300-second
limits and have zero scored routing violations. Workbench B: main U 82,
320 vias, 6,803.0864 mm, CPU 300.50; smoothing U 94, 311 vias, 6,866.6176 mm,
CPU 300.33. G: main U 87, 315 vias, 6,841.1207 mm, CPU 300.27; smoothing
U 95, 311 vias, 7,014.9829 mm, CPU 299.69.

Downloaded original/corpus files for both revisions. Their originals are fully
connected with zero routing DRC, using two layers and a 144-pin LQFP FPGA
(U6, 0.5 mm pitch, 0.3 mm-wide pads). B: 156 nets, 9,921.6393 mm, 555 vias,
front/back lengths 5,089.3032/4,832.3361 mm. G: 206 nets, 10,188.6705 mm,
776 vias, front/back 5,027.6181/5,161.0524 mm. G also has a 48-pin TSOP
with 0.25 mm-wide pads, versus B's 44-pin 0.55 mm-wide TSOP pads. Lower
router wirelength/via totals are not a success metric on these partial routes.

Started `quality-smoothing-karabas-pilot`: main, smoothing, smoothing+empty
proposal guard, both boards, ten passes/300 seconds/one thread/four jobs.
Early two-second profiles (`karabas-{B,G}-smoothing.sample.txt`) show batch
routing and maze expansion work: 780/934 and 869/928 active samples in the
pass runner. These samples do not establish a repeated tightening loop like
CoreOne. Pilot results pending; no new hypothesis patch made.

At 736 attempts smoothing full has 733 scored PCBench plus two Java fixtures.
PCBench: twelve U-improved / twelve regressed, ΔU +41, two DRC gainers,
ΔV -1, CPU ratio 0.9991. Latest reduction real-time-chess -8 V; gainers azalea
+12 and saiboard +2. Djinn remains the only new missing output. Still partial.

### Baseline clearance-rule provenance audit

Re-read all downloaded PCBench baseline referee reports, selecting error
severity. Of 256 scored `clearance` errors: 109 explicitly cite pad-local
clearance, four footprint-local clearance, 34 board-minimum clearance,
37 named net-class clearance, and 72 have an unlabeled clearance source.
Of 48 `hole_clearance` errors: 34 cite pad-local clearance and fourteen cite
board hole constraints. Thus 143/921 baseline routing errors explicitly cite
pad-local rules; this count alone does not prove every rule was omitted from
its board's DSN. The individually inspected missing-input examples remain
the evidence for that mechanism, rather than attributing all 921 errors to it.
Other explicit global constraints: all 250 track-width errors cite board
minimum width; all eleven via-diameter errors cite board minimum diameter;
132/246 edge-clearance errors cite a board edge-clearance value (114 do not).
No baseline referee categories or scoring rules changed.

Karabas pilot's four main/smoothing cells completed: B main U 32/V 0/CPU
298.46 versus smoothing U 32/V 0/CPU 297.61; G main U 49/V 0/CPU 298.27
versus smoothing U 36/V 0/CPU 297.45. All internally TIMED_OUT, all referees
scored. Local smoothing thus has one U-improved / zero regressed, ΔU -13,
zero DRC gainers, ΔV 0, CPU ratio approximately 0.9972. The workbench
regressions were not reproduced on these local deadline-limited runs.
The two repaired-variant cells are now running; do not infer the CoreOne
empty-proposal mechanism from these boards' remote aggregate deltas alone.

### Smoothing PCBench cells complete; sizif512ext regression under investigation

All 740 PCBench attempts finished: 739 scored pairs (Djinn newly missing),
thirteen U-improved / thirteen regressed, ΔU +56, two DRC gainers, ΔV -1,
CPU ratio 1.0078. Java fixtures are still running, so the overall export is
pending. The final large U regression is sizif512ext: main U 43/V 0/CPU
299.48 s, smoothing U 69/V 0/CPU 299.64 s. Both internally TIMED_OUT.
Main has 268 vias/9,819.619 mm; smoothing 224 vias/8,170.6175 mm.

Downloaded its original and input files. Original: 179 nets/two layers,
zero unrouted/zero routing DRC (212 all-category errors), 10,955.947 mm,
294 vias. Front/back routed lengths 6,785.2328/4,170.7142 mm. Dense parts
include U7 TQFP144 with 0.3 mm-wide pads and U11 QFN48 with 0.2 mm-wide
pads; several DIP40 parts and a 50-pad double-sided edge connector also
consume routing space. Started `quality-smoothing-sizif-pilot`: main,
smoothing, repaired smoothing, ten passes/300 seconds/one thread/three jobs.
An early sample (`sizif-smoothing.sample.txt`) has 1,539/1,576 active samples
in the pass runner, including maze expansion/via placement checks; it does
not establish CoreOne's empty-proposal loop. No new implementation change.

For context, the completed workbench nominal and nominal+smoothing runs have
sizif U 54/V 0 in both. Karabas B has U 67 then 56 (main 82); G has U 70
then 80 (main 87), all V 0. Candidate interactions are board-specific;
standalone smoothing's regressions cannot simply be added to nominal's totals.

Karabas pilot completed all six scored cells. Repaired smoothing: B U 32/V 0/
CPU 297.65, G U 36/V 0/CPU 297.73, both internally TIMED_OUT. Relative to
smoothing, zero U-improved / zero regressed, ΔU 0, zero DRC gainers, ΔV 0,
CPU ratio 1.000538. Relative to main, one U-improved / zero regressed,
ΔU -13, zero DRC gainers, ΔV 0, CPU ratio approximately 0.99774.
The guard provides no additional connectivity benefit on this local pilot;
CoreOne remains its demonstrated quality recovery. Do not generalize that
recovery to other deadline regressions without corpus evidence.

Sizif pilot completed all three scored KiCad cells, all internally TIMED_OUT:
main U 39/V 0/CPU 296.15; smoothing U 39/V 0/CPU 295.83; repaired smoothing
U 39/V 0/CPU 296.17. Both candidates versus main: zero U-improved / zero
regressed, ΔU 0, zero DRC gainers, ΔV 0; CPU ratios 0.998919 and 1.000068.
The workbench +26 U regression was not reproduced locally, and the guard
shows no quality effect in this pilot. No additional patch is justified by
these measurements. Full workbench validation remains authoritative.

### Smoothing supplementary KiCad audit and candidate preservation

All 739 scored PCBench pairs audited by error severity (Djinn excluded):
clearance 256→275 (+19; twelve type-specific gaining boards), edge clearance
246→238 (-8), hole clearance 48→43 (-5), shorts 72→65 (-7; eight type-specific
gainers). Their aggregate routing delta is -1, with two boards gaining total
routing DRC. Solder-mask bridge errors 13,748→13,734 (-14; eight gaining
boards); all-category error delta -15. This differs from nominal's mask-error
increase and should not be conflated with it. Script:
`/tmp/quality-compare-referee-types.py quality-smoothing-full smoothing`.

Archived exact current candidate source in `/tmp/copperroute-quality`:
`census-reviewed.patch`, `census-reviewed-files.{zip,json}` (one file), and
`smoothing-noop-reviewed.patch`, `smoothing-noop-reviewed-files.{zip,json}`
(three files). Manifests record main base SHA and SHA256 for each source file.
Patch SHA256 respectively:
`8d1f0779171232eab98e4d82b7fa000662f9404f2bf4be37c625f8555c0f6ddc`,
`165b1864aeb44996b723a9cb34a56021c40c5144b9507627dd9d030bfbdfa565`.
These preserve the tested versions without committing candidates before full
validation. Remote active/queued process handles verified live; no restarts.

### Standalone smoothing full export complete; census full run started

`quality-smoothing-full` exported all 845 attempts. Downloaded
`quality-smoothing-full-smoothing-636e039.json` into `/tmp/copperroute-quality`.
Referee audit: four unchanged baseline Java missing outputs, plus new Djinn
missing output. Excluding those five gives 840 scored pairs:

| Referee | Paired boards | U improved / regressed | ΔU | DRC gainers | ΔV | CPU ratio | Benchmark clean passes |
|---|---:|---:|---:|---:|---:|---:|---:|
| KiCad / PCBench | 739 | 13 / 13 | +56 | 2 | -1 | 1.007821 | 479→478 |
| Java fixtures | 101 | 10 / 5 | -62 | 0 | 0 | 1.011732 | 22→22 |

PC U 5,174→5,230, V 921→920, CPU 36,270.82→36,554.50 s.
Java U 2,646→2,584, V 19,295→19,295, CPU 12,438.24→12,584.16 s.
Scored aggregate ΔU -6/ΔV -1 does not erase Djinn's lost output. The late
Java improvement includes issue754-avionics_hub -17 U. KiCad supplementary
all-error delta remains -15 including mask bridges -14, as audited above.
Hold standalone smoothing uncommitted: PC connection quality is worse and
output failure persists. This is a measured aggregate trade, not rejection
merely because a small number of boards regressed. The empty-proposal repair
has a confirmed local CoreOne mechanism/recovery and awaits its own full run.

Four full experimental runs have now completed (excluding baseline/sanity).
Census-only started automatically, bench PID 4111696; combined-census and
smoothing+empty-proposal-guard remain queued. Verified the queued census and
combined-census SHA256 values still match their tested binaries. Initial
census KiCad referee reports are `status: ok`, with real category counts
(e.g. ABOVISP has thirteen all-category errors), project used and successful
SES import. No environment omission or fake all-zero referee observed.

### Physical context of CoreOne's empty proposal

Matched the exact captured coordinates to the scored smoothing SES and KiCad
output. Trace 178007 is `/XMOS200/DVDD` on In2.Cu, width 0.2 mm; its triangle
has 1.6 µm orthogonal sides and starts at (138.7465,88.3121) mm, exactly the
center of a same-net via with 0.8 mm diameter and 0.5 mm drill. Even its
furthest copper extent is bounded by 0.1 + sqrt(2)*0.0016 = 0.102263 mm
from the center, inside the 0.25 mm drill radius. Retaining this tiny closed
trace does not add physical copper outside that via in this example. The
repair's purpose remains truthful progress reporting, not deleting routes.

The original DVDD routing itself includes a 0.5487 µm segment on F.Cu, so
small length alone is not evidence for a safe global cleanup threshold.
No generic minimum-segment rule was introduced. This supports keeping the
six-line guard narrowly tied to the demonstrably unapplied empty proposal.

### New root cause: relay-board fanout reports zero-distance via moves

Ranked main's remaining KiCad connection deficits against Java. The largest
is `esp32-4-channel-relays`: main U 53/V 0 versus Java U 10/V 0. Nominal,
images, nominal+smoothing, standalone smoothing and the previously landed
neckdown export all also have U 53 at the 300-second deadline. Original is
fully connected with zero all-category DRC, 90 nets/two layers, 841.1331 mm,
34 vias, twenty refilled zones. DSN default width/clearance 0.25/0.2 mm;
power class 0.5/0.175 mm. Downloaded its original and input files.

Local `quality-relay-baseline-pilot` reproduced main exactly: U 53/V 0,
CPU 296.91 s, internally TIMED_OUT, 27 vias/262.2 mm, KiCad referee scored.
Early profile (`relay-main.sample.txt`) is in fanout, with 820/975 active
samples in via optimization. An existing OCA-ledger diagnostic counted
976,150 successful moves on via 1766 in ninety seconds, while successful
trace tightening stopped at 54. This diagnostic was intentionally terminated
at ninety seconds; it is not a benchmark failure. Artifacts:
`count-relay-tightening.py`, `relay-ledger-{progress.log,summary.json}`.

Created detached helper `copperroute-quality-via-progress` at main. A separate
diagnostic build confirmed repeated identical proposed/current coordinates:
via 1766 at (1422906,-1110563), trace 1773, next corner (1420740,-1108397).
`opt_plane_or_fanout_via` lacks the existing general optimizer's guard against
new location equal to current location. A positive movement allowance can
round back to that location; DrillItemMover accepts the zero vector, and the
caller reports true, restarting changed-area optimization. Temporary env-gated
instrumentation was removed from source after building the diagnostic binary.

Failing-first test `fanout_via_does_not_report_a_move_that_rounds_to_its_current_location`
uses two square vias and a tiny trace. After correcting the fixture for the
existing sixteen-unit safety margin, its positive allowance is
105*sqrt(2)-(31+100+16+1)=0.4924 units, which rounds to zero movement. The red
run proved that allowance, an unchanged proposed/current location and unchanged
via center, then failed on the incorrect successful-move return
(`via-progress-red.log`). Added the same equal-location guard already used
by the general optimizer, preserving existing movement checks. Both via
optimizer targets pass, including JVM recordings (`via-progress-green.log`).
Release build and full workspace tests running; real repaired-board and corpus
measurements remain required before acceptance or any commit.

### Relay repair measured: 53→8 KiCad-unrouted with zero violations

The isolated via-progress release built successfully. `quality-relay-via-progress-pilot`
completed and KiCad scored it: U 8/V 0, CPU 5.18 s, 35 vias/801.6796 mm,
COMPLETED. Versus local main U 53/V 0/CPU 296.91 s/TIMED_OUT: one
U-improved / zero regressed, ΔU -45, zero DRC gainers, ΔV 0, CPU ratio
0.017446. Compared with the original 841.1331 mm/34 vias, the repaired
partial route has length ratio 0.9531 and via ratio 1.0294. Internal reporting
says four unrouted, but **KiCad says eight** and is the authoritative figure.
This is a focused-board result, not yet a corpus-wide acceptance claim.

Started all eleven local KiCad fixtures (`quality-local-via-progress`) and the
full workspace suite (`workspace-via-progress.log`), both still running.
Prepared a separate remote helper at main and copied only the two changed
via-optimizer source/test files. Detached release build PID 4128392,
log `/tmp/quality-build-via-progress.log`. No running benchmark binary was
changed. Full via-progress run has not yet been queued. Given its isolated
mechanism and large local gain, consider validating it immediately after the
active census run; any queue reorder must preserve already-running work.

### Via guard passes local gates and is next in the full-run queue

`quality-local-via-progress` completed eleven scored KiCad cells: one
U-improved / zero regressed, ΔU -11 (250→239), zero DRC gainers, ΔV 0
(40→40), CPU 296.85→312.30 s, ratio 1.052046, clean passes 6→6. The local
batch CPU increased; the relay's dramatic speedup must not be generalized.
`cargo test --workspace` completed successfully: 2,555 passed, zero failures,
zero warnings (`workspace-via-progress.log`). No JVM recordings changed.

Remote release build completed in 2m29s; both source/test SHA256 values match
local. Binary SHA256:
`c4ab016d6af3e3178b62547003bea937a80af2d48ad6a74eb199354c9d429e06`.
Archived `via-progress-reviewed.patch`, `via-progress-reviewed-files.{zip,json}`;
patch SHA256 `1eaccda790f504c81eebeeba2af9f17d5a5ac12ea16c0776ef3d3a6841751952`.

Promoted this isolated, reproduced fix ahead of the pending composition runs.
Verified census is still running (PID 4111696) and its export does not exist;
verified combined-census's old waiting bash PID 4048256 had only a `sleep 30`
child, then terminated that waiting shell. No active routing run was stopped.
Queued `quality-via-progress-full` immediately after census, and requeued the
unchanged combined-census binary after via-progress's export. The existing
smoothing-noop queue still waits for combined-census. All use 845 boards,
ten passes/300 seconds/one thread/twelve jobs with explicit referee variables.

Current order: **census → via-progress → nominal+smoothing+census →
smoothing+empty-proposal guard**. Verified waiting script PIDs 4134979,
4135106 and 4096966, respectively; original combined waiting PID is gone.
New scripts/config: `/tmp/quality-run-via-progress.sh`,
`/tmp/quality-via-progress-candidates.toml`,
`/tmp/quality-run-combined-census-after-via.sh`. Four full experiments completed,
one active, three queued. No candidate committed before its full validation.

### Starling: a separate via-optimization stall

Examined the original Starling_Starling_V1 board: 43 nets, two layers, 58
vias, 1565.7457 mm routing, two zones, zero original unrouted connections
and zero all-category DRC errors. Main's workbench result is 17 U / 0 V,
298.79 CPU seconds. Nominal clearance completed with 0 U / 0 V in 77.63
CPU seconds; nominal+smoothing also completed cleanly in 81.51 seconds.

Local paired `quality-starling-via-progress-pilot` completed and was scored:
main 17 U / 0 V / 297.80 CPU seconds; via-progress 17 U / 0 V / 298.58
seconds. Both hit the router time limit. Zero improved/regressed boards,
ΔU 0, zero DRC gainers, ΔV 0, CPU ratio 1.002619. Both have 50 vias and
863.2591 mm routing, substantially less than the completed original because
connections remain missing. The relay guard does not fix this board.

A main-process sample concentrates in via optimization during changed-area
optimization in the routing passes. A bounded 60-second counter diagnostic
(`starling-ledger-summary.json`) counted 130,269 successful changes to via
2840, versus at most seven to any other via, with a repeatedly unchanged
changed-area region. The separate equal-proposed/current-position diagnostic
emitted no matches. This distinguishes the symptom from the relay's proven
zero-distance move. Preparing a bounded position diagnostic to determine
whether the general optimizer cycles between different locations; no new
production fix inferred from the counter alone.

Census full-run interim check: 371 scored PCBench boards, one improved and
zero regressed, ΔU -4 (ReSDMAC), zero DRC gainers, ΔV 0, CPU ratio 0.9802,
no new unscored outputs. This is partial evidence, not a landing decision.

### Census interim regression: investigate Amalthea rather than reject by count

At 396 scored PCBench boards, census has one improved / one regressed,
ΔU 0, zero tracked DRC gainers, ΔV 0, CPU ratio 0.9789. ReSDMAC improves
four connections; Amalthea regresses four (40→44 U). Both Amalthea results
hit the router deadline, but main reaches five passes and census ten.
Main/census CPU is 304.00/300.11 seconds, RSS 67.1/91.2 MB, vias 181/170,
wirelength 1814.5552/1746.9667 mm. Both retain the same 201 solder-mask and
one courtyard errors; routing V remains zero. Internal U is 31/34, whereas
the referee scores 40/44. More completed passes do not guarantee a better
final route. The original is complete with 1866.0593 mm, 264 vias, four
layers and the same 202 all-category errors. A prior local main result was
also 44 U, but is not a matched census comparison. Started paired local
`quality-census-amalthea-pilot` with main and census, 300 seconds / ten
passes / one thread / two jobs, to test reproducibility. No rejection based
on this single regression.

### Starling position trace confirms a one-grid-unit cycle

The general-via position probe emitted no records for via 2840. A separate
fanout probe (`starling-fanout-positions.log`, bounded to 24 records) captured
the stall: repeated moves from (1366119,-765089) to (1366118,-765089), then
back to (1366119,-765089). The first move consumes an adjacent trace corner;
the reverse move has next corner (1366118,-765088) and the preceding diagonal
from (1357463,-756433). The rounded perpendicular-projection fallback is a
specific lead for that reverse move. Both proposals change position, explaining
why the already-tested zero-distance guard does not help. Next step is to
reproduce this geometry in a failing test and establish which fallback branch
returns the reverse proposal before selecting a guard. No generic minimum
movement threshold is justified by this evidence. Temporary instrumentation
was removed from the helper source after building the diagnostic binary;
queued production binaries were unchanged.

### Amalthea paired census pilot: same completed route, faster census

`quality-census-amalthea-pilot` completed both KiCad-refereed cells. Main and
census each finish ten passes with 44 U / 0 V, 170 vias, 1746.9667 mm and
identical internal geometry statistics. Zero improved/regressed, ΔU 0,
zero DRC gainers, ΔV 0; CPU 189.19→155.30 seconds, ratio 0.820867.
RSS increases 103.6→119.3 MB. Both local reports have 199 mask errors and
one courtyard error (the workbench reports have 201 mask errors); compare
within environment. This supports a deadline-dependent workbench difference:
main stopped after five passes at 40 U, while census reached the ten-pass
route at 44 U. Keep the full-run +4 regression in reported numbers; the
paired evidence explains why it is not grounds for automatic rejection.

### Starling projection guard: failing-first geometry regression

Created isolated helper `copperroute-quality-via-projection` at main, without
the separate zero-distance guard. Extracted the existing rounded projection
calculation unchanged and tested Starling's actual via/corner/previous-corner
coordinates. The red test returned (1366119,-765089), precisely the observed
reverse move, instead of refusing it (`via-projection-red.log`). Added a guard
requiring the rounded projection to reduce squared distance to the next
corner, a property of an exact nontrivial perpendicular projection that
integer rounding can destroy. Existing clearance and angle guards remain.
The captured regression and a useful shortening projection both pass
(`via-projection-green.log`). Both existing via optimizer integration targets
pass, including JVM recordings (`via-projection-parity.log`); no recordings
changed. A first command named a nonexistent angle-restriction test target;
the corrected invocation ran `via_optimizer` and `via_optimizer_reposition`.
Release build and actual Starling validation follow; geometry tests alone
are not evidence of routing improvement.

### Census interim: Azalea adds clearance errors while connecting more nets

At 467 scored PCBench boards: four improved / one regressed, ΔU -21,
one DRC gainer, ΔV +15, CPU ratio 0.9733, no newly missing outputs.
Azalea contributes -11 U and all +15 V. Downloaded its census referee
report (`azalea-artifacts/census-referee-drc.json`): all 55 clearance errors
require pad-local 0.1778 mm; actual gaps span 0.1541–0.1758 mm, with 30
at 0.1541. This matches the previously established omission of Azalea's
pad-local clearance from its DSN (0.1524 mm). There are also 159 mask
errors. More routing progress exposes more errors under the original
KiCad constraints; these remain real violations and count against the
candidate. Do not describe census as DRC-neutral from earlier partials.

Starling projection release build succeeded; started the actual 300-second,
ten-pass, single-thread pilot `quality-starling-via-projection-pilot` and
`cargo test --workspace` (`workspace-via-projection.log`). Neither result
is assumed from the focused geometry tests.

### Starling projection guard succeeds in the actual board pilot

`quality-starling-via-projection-pilot` completed with a clean KiCad referee:
0 U / 0 V, 42.47 CPU seconds, ten passes, 38 vias, 1385.0434 mm.
Against the paired baseline captured immediately before the investigation
(17 U / 0 V / 297.80 CPU seconds), one improved / zero regressed, ΔU -17,
zero DRC gainers, ΔV 0, CPU ratio 0.142612. Baseline timed out; candidate
completed. Original board: 58 vias / 1565.7457 mm / 0 U / 0 all-category
DRC; candidate wirelength ratio 0.8846 and via ratio 0.6552. This is a
standalone projection fix at main, not composed with nominal clearance or
the separately queued zero-distance via guard. Started eleven local KiCad
fixtures (`quality-local-via-projection`) and prepared a new isolated
remote source worktree for full-corpus validation. No full run queued yet.

### Projection guard passes all eleven local KiCad cells

`quality-local-via-projection` completed with all eleven referee statuses
`ok`: 243 U / 40 V / 310.17 CPU seconds, six clean passes. Versus main:
one improved / zero regressed, ΔU -7 (proba 16→9), zero DRC gainers,
ΔV 0, CPU ratio 1.044871. The 4.5% batch CPU increase is reported alongside
the separate Starling speedup; do not generalize from that single board.
Workspace tests are still running, including the byte-identical CI routing
check. Remote build PID 4153068 remains under observation.

Archived `via-projection-reviewed.patch` and matching file zip/manifest;
source SHA256 `522a30a2bac4bad8c7f2e700fe51e15fc38f417544d947fe5cc63d8dc5110fab`,
patch SHA256 `5e6fb27593a5ab35822ec6b4d0054ab3df01b9d44aabedca43a272fd7518157a`.
Prepared a full-run script to wait for smoothing-noop's completed export,
with explicit Java/KiCad referee environment and the standard 845-board,
300-second/ten-pass/one-thread/twelve-job configuration. Script is not yet
launched; queued and active binaries have not been modified.

### Projection guard queued for full validation

Remote release build completed in 2m20s; source SHA256 matches the archived
local file. Binary SHA256:
`dcb1a6bf5fd3035bae392dfdf058925a820617e5ebb503d566cf8cd520d00944`.
Queued `quality-via-projection-full` after smoothing-noop's export, verified
waiting bash PID 4156020 and wrapper 4155979. Queue is now census (active),
via-progress, nominal+smoothing+census, smoothing-noop, via-projection.
Four completed full experiments, one active, four queued. No active run
was stopped and no queued binary overwritten. Workspace test execution
finished; its final totals are recorded below after checking the exit status.

Projection `cargo test --workspace` exited 0: **2,556 passed, zero failed,
zero warnings**, including the byte-identical CI routing check. No commit
before full-corpus validation.

### Nominal mask regression: original LPC2148 comparison

Downloaded original `LPC2148_Stick_LPC2148_stick` and nominal output/referee.
Original: 97 nets, two layers, 0 U / 0 routing V / 72 solder-mask errors,
5513.9113 mm and 116 vias. Main has 130 mask errors; nominal 199 (+69).
Original setup explicitly expands pad mask apertures by 0.2 mm. Its DSN
has copper clearance 177.8 µm (44.45 µm for smd_smd) and no mask rule.

A nominal error joins P1 pad 38 [/TXD0] at (72.009,86.36) mm and a +5V
track from (70.6391,86.7988) to (73.686,89.8457), width 0.254 mm. Pad is
oval 2.032×1.7272 mm. Capsule-to-segment distance gives copper gap
0.18058096 mm, legal under the DSN's 0.1778 mm. Its mask aperture extends
0.01941904 mm over the track, matching KiCad's mask-bridge error. The
calculation has the same result for either horizontal/vertical oval axis
because this track is diagonal. Original's nearest explicit +5V F.Cu track
runs from (70.2564,88.646) to (117.475,88.646), width 0.508 mm, with at
least 1.016 mm copper gap under either oval orientation. This is a concrete
original-routing comparison, not an assumption that mask errors are harmless.
Artifacts are in `lpc-mask-audit/` and `LPC2148_Stick_LPC2148_stick/` under
`/tmp/copperroute-quality`. The already downloaded `pcbench-LPC2148...`
referee was main's, so downloaded nominal separately before measuring.

This proves one gained mask error comes from a constraint absent in the
DSN, not a copper clearance breach. It does not establish the cause of all
333 gained nominal mask errors and does not remove them from the trade-off.
Do not invent a global mask margin for DSN inputs that lack this rule.

Census interim at 521 scored PCBench boards: six improved / two regressed,
ΔU -34, one DRC gainer (Azalea), ΔV +15, CPU ratio 0.9813, no newly
unscored outputs. New U changes include Karabas C -17, A -2, B +6.

### Prioritize the two reproduced via fixes for full validation

Promoted projection immediately after via-progress, ahead of the broader
composition candidates. Verified active census PID 4111696 and both old
waiting shells had only `sleep 30` children, with their awaited exports
absent. Terminated only waiting bash PIDs 4135106 and 4156020. Requeued
unchanged projection binary with `/tmp/quality-run-via-projection-after-via.sh`
(new waiting PID 4165166), and unchanged triple binary with
`/tmp/quality-run-combined-census-after-projection.sh` (PID 4165169).
Verified old waiting PIDs are gone, census still live, via-progress waiting
PID 4134979 and smoothing-noop waiting PID 4096966 unchanged. All referee
environment and bench settings remain intact. Current order: census →
via-progress → via-projection → nominal+smoothing+census → smoothing-noop.
This changes priority only; four full runs completed, one active, four queued.

Recomputed all four completed comparisons from exports and successful referee
reports (`/tmp/quality-summarize-completed.py`); results exactly match the
previously reported counts. Added `docs/routing-quality-results.json` and a
comparison table near the start of this log, including missing outputs and
supplementary KiCad mask/all-error deltas. Paired Karabas B census pilot
`quality-census-karabas-b-pilot` remains live at this update.

### Karabas B census regression does not reproduce locally

`quality-census-karabas-b-pilot` completed with both KiCad referee statuses
`ok`. Main and census each hit the 300-second router limit in pass eight
with 32 U / 0 V. Zero improved / regressed, ΔU 0, zero DRC gainers,
ΔV 0, CPU 295.11→294.80 seconds, ratio 0.998950. Geometry differs:
373→375 vias and 8906.0145→8970.8562 mm; RSS 84.5→89.8 MB. Thus this
is not proof of identical intermediate routing, but the workbench +6 U
regression does not reproduce in the matched local run. Preserve +6 in
the full-run comparison. Original board was previously inspected: complete,
zero routing DRC, 9921.6393 mm and 555 vias. Neither local partial route
should be called more efficient merely because it has less routing.

### Census supplementary DRC audit at 655 scored PCBench boards

Eight U-improved / two regressed, ΔU -42, one net routing-DRC gainer,
ΔV +15, CPU ratio 0.9834, no newly unscored outputs. Error-severity
referee maps give clearance +16 (seven type gainers), shorts -1 (seven
type gainers), mask bridges -19 (six type gainers), all-error delta -4.
Thus total KiCad errors decrease slightly at this partial checkpoint, while
routing errors increase; report both rather than allowing mask reductions
to conceal clearance gains. Actual CLI versions: local KiCad 10.0.3,
workbench 10.0.4. This is a possible contributor to cross-host mask counts,
not proof of their cause. All primary comparisons remain within host.

Checked the proposal to preserve a best board across passes after observing
Amalthea's earlier deadline result beat its ten-pass result. This mechanism
already exists: `batch_loop.rs` adds pre-pass snapshots to `BoardHistory`,
and `final_best_board_swap` restores the lowest-penalty stored board.
History retains up to 30 snapshots, preferentially retaining lower penalty.
Do not add a redundant best-board cache based solely on this symptom.
A deadline can stop within a pass, at a state not captured at a pass boundary;
no new change is justified without a finer-grained reproduction.

### Census late PCBench gain: nonSNES and its new copper-graphic error

At 726 scored PCBench boards: eleven U-improved / two regressed,
ΔU -201, two routing-DRC gainers, ΔV +16, CPU ratio 0.9820,
no newly unscored outputs. nonSNES contributes -157 U and +1 V.
Its scored candidate is 162 U / 1 V / 301.57 CPU seconds, router timed
out in pass two; internal U=92 must not replace KiCad's 162. Candidate
has 280 vias / 6869.4094 mm / 47.9 MB RSS. Original board: 245 nets,
two layers, fifty zones, 0 U / 0 all-category DRC, 686 vias and
12503.2861 mm. Less candidate wire is incomplete routing, not evidence
of greater efficiency. Files downloaded under `nonSNES_SNSP-CPU-1CHIP/`
and `nonsnes-census-audit/` in `/tmp/copperroute-quality`.

The new error is a direct overlap of /PA5 F.Cu trace at x=150.5724 mm,
y=96.0249→56.7215 mm, width 0.25 mm, with U2's filled F.Cu rectangle
UUID cd681d3e-cb38-4696-bd6c-9f78d5bee1d0. U2 is at
(137.3118,70.9552), rotated 90°; its local rectangle spans
(-10.5,12.5)→(-9.5,13.5), with 0.2 mm stroke. Global stroked extent
is x=149.7118→150.9118, y=80.3552→81.5552 mm. Candidate crosses it.
Original's nearest explicit /PA5 F.Cu segment is over 49 mm away;
its original routing is DRC-clean.

DSN image `SNES_Library:PQFP-100_14x20mm_P0.65mm` includes the rectangle
as `(outline (path signal 200 -10500 -12500 ...))`, alongside four
other outlines, with zero keepouts. The export loses the actual copper
layer and filled-region semantics. This explains a concrete new error;
it is not a reason to reinterpret every image outline as a copper obstacle
or remove the violation from the quality comparison.

### Prepare interaction tests for census and both via fixes

Created detached helper `copperroute-quality-census-via-guards` at main.
Copied the exact census statistics source and projection-guard source, then
applied the already-reviewed zero-distance patch after `git apply --check`.
No conflict resolution or new algorithm was required; `git diff --check`
passes. The composition contains the three reproduced mechanisms and their
existing failing-first regression tests. Started its release build and full
workspace suite in independent target directories. Added local candidate
`census-via-guards` for the eleven fixtures and the two reproduced stall
boards after the build. This composition is not yet queued on workbench;
the four individual/pending full runs are unchanged. Its purpose is to
validate the cumulative behavior before any eventual combined landing.

### Census completes all 740 PCBench referee results

All 740 scored, no newly missing outputs: twelve U-improved / three
regressed, ΔU -181, two net routing-DRC gainers, ΔV +16, CPU ratio
0.9894. Final KiCad type audit: clearance +13 (eight type gainers), shorts
+3 (eight type gainers), mask bridges -6 (ten type gainers), all-error
Δ+10. These supersede the earlier partial -4 all-error snapshot.
The third U regression is Sizif, 43→68 U, 0 V, 299.15 CPU seconds,
router timed out in pass four, 231 vias / 8203.7408 mm / 102.5 MB RSS.
Started matched local `quality-census-sizif-pilot` with main and census,
300 seconds / ten passes / one thread / two jobs. Its original and earlier
smoothing-focused pilots are already documented above; do not substitute
them for a direct census comparison. At this checkpoint 27 Java pairs
have scored: one improved / zero regressed, ΔU -1, ΔV 0, CPU 0.9718.
Full census run remains active and has not exported yet.

### Combined stall pilot preserves both via-fix gains

`quality-census-via-guards-stalls` completed with valid KiCad referees.
Relay: 8 U / 0 V / 4.58 CPU seconds; Starling: 0 U / 0 V / 48.63 CPU
seconds, both completed. Against the previously measured main pilots:
two improved / zero regressed, ΔU -62, zero DRC gainers, ΔV 0,
CPU 594.71→53.21 seconds (ratio 0.089472). These are separate local
pilot comparisons, with other local validation running concurrently;
full-corpus CPU remains the acceptance evidence. The eleven-board
composition suite and workspace tests remain in progress.

### Combined eleven-board result exposes a Proba regression

All eleven `quality-local-census-via-guards` referees are `ok`: 252 U /
40 V / 303.78 CPU seconds, six clean passes. Zero improved / one regressed,
ΔU +2, zero DRC gainers, ΔV 0, CPU ratio 1.023345. Proba is the sole
change, 16→18 U with three routing violations unchanged. Both main and
composition timed out in pass three (90.54/89.90 CPU seconds in the
120-second wall-limited pilots). Standalone zero-distance guard previously
had 5 U in pass five at 112.04 CPU seconds; projection guard had 9 U in
pass four at 106.30 seconds. Those separate pilots do not establish additive
quality gains under the deadline. Next test is a matched longer Proba run,
not rejection based merely on one regressing board. No combined full run
has been queued yet; individual full runs remain unchanged.

Combined `cargo test --workspace` exited 0: **2,558 passed, 0 failed,
0 warnings** (`workspace-census-via-guards.log`). Local Proba regression
remains under investigation despite passing tests.

### Additional temporary benchmark server authorized by user

User supplied `root@185.189.44.159` as an additional experiment resource,
explicitly requiring results be copied to laptop or workbench because it
may be powered off at any time. Verified Ubuntu 24.04.4, AMD EPYC 9654,
96 physical cores / 192 SMT threads, 755 GiB RAM, 3.3 TiB free disk.
Installed Nix 2.18.1 and transfer utilities. Initialized a separate checkout
from a git bundle of main 636e039. Exact workbench KiCad 10.0.4, Python
3.14 pcbnew environment, JDK25, uv/Python312, xvfb-run, GNU time and Rust
binary runtime closures copied using Nix's signed substitutes/closure copy.
Corpus transfer dereferences workbench's PCBench symlink (3.8 GiB actual
corpus, not its misleading 14 MiB directory size). Immutable main and five
candidate binaries are being copied; no running workbench run stopped.

Generated a dedicated SSH transfer key on workbench, authorized its public
key with `restrict` on the temporary server. No private key was copied.
Workbenched pull loop `/tmp/quality-server-backup-loop.sh` (PID 4179845)
copies results, exports and logs to `~/copperroute-server-backup` every
30 seconds, retrying transient failures. Verified marker file reached
workbench before launching any benchmark. Backup target has 912 GiB free.
Server setup/copy logs are on workbench under `/tmp/quality-server-*.log`.
A matched main baseline and referee canary are required before accepting
cross-host candidate results. Provisional estimate supplied to user:
15–25 minutes for one full Rust corpus run with substantial server CPU
allocation, excluding setup; concurrent experiments share that capacity.

### Final census decision and primary-worktree gate

Full run has 845 attempts, 842 scored outputs, 841 matched pairs. PCBench:
12 improved / 3 regressed, -181 U, two DRC gainers, +16 routing V,
CPU ratio 0.9894369111. Java matched: 10 improved / 0 regressed,
-216 U, zero DRC gainers, ΔV 0, CPU ratio 0.9842156125. Three previously
unscored fixtures remain without output; issue756-tomu-fpga8 newly produces
11 U / 853 Java-DRC V / 342.12 CPU seconds. It is excluded from paired
gains and is not called a clean recovery. No new missing outputs.

Matched local Sizif census pilot finished: main and census both 38 U / 0 V,
10 passes, timed out, 294.88/294.90 CPU seconds. Its workbench +25 U
regression does not reproduce locally, but remains counted. Longer matched
Proba composition pilot also finished: main and census-via-guards both
0 U / 3 V, ten passes COMPLETED, 302 vias / 6917.8524 mm;
255.87/254.33 CPU seconds. The 120-second composition regression does not
persist at 300 seconds. Original Proba has 225 nets, four layers, 247 vias,
6689.3196 mm and zero DRC errors; candidate's three persistent errors are
not dismissed. Do not claim the original's routing quality has been matched.

Selected census for landing: 397 fewer matched unrouted connections across
22 improved boards versus three regressing boards, while the 16 gained
KiCad routing errors are confined to two already-incomplete boards whose
missing input constraints were individually established. This is a measured
trade, not a DRC-neutral claim. Memory increases are documented in pilots.
Verified all 25 held nominal/smoothing source and test-data hashes against
the archive before restoring 16 tracked files and removing nine archived
untracked files. Applied only census-reviewed.patch in the primary worktree;
its statistics.rs SHA256 matches the fully benchmarked helper exactly.
Started required `cargo test --workspace` in the primary worktree, log
`workspace-primary-census.log`; no commit before its successful completion.

### Temporary server now running full validation with verified backups

Four-board canary matched workbench exactly: ATtiny461 0 U / 1 V,
constant_current_ac_hv 0 U / 8 V, LTC6802 0 U / 2 V, and Java-refereed
issue026 0 U / 0 V. All referee statuses ok; exports verified on workbench.
Corpus transfer completed (4,006,872,223 bytes); all six copied binary
hashes match. Launched `quality-epyc-full-01`, bench PID 20845, 845 boards
per candidate, six candidates, max ten passes / 300 seconds / one routing
thread / 96 board jobs. Wrapper/script `/root/server-full.sh`, runtime
`/root/server-env.sh`, candidates `/root/server-candidates.toml`, logs
`/root/copperroute-logs/`. Exact baseline hash:
`6d7bce8042a5a43e6920e290668b7335f4ba5c428b81fcff30c951b60b7ef447`.

After verifying the new run live, stopped only the three old waiting shells
4165166, 4165169, 4096966, all previously verified to have sleep children.
The workbench via-progress run continues independently. No active routing
was killed. Added a separate metadata backup loop on workbench using
`/tmp/quality-server-metadata-backup.sh`; small JSON/log/export files go to
`~/copperroute-server-backup/metadata` without waiting behind large board
artifacts. Original artifact backup loop remains active. Verified more
than 500 new-server cell directories already present in the metadata mirror.

User asked about ~50% CPU. A three-second /proc/stat/topology sample found
50.9% across 192 logical CPUs, but 95/96 physical cores had >50% combined
activity, median combined core activity 100.3%, with no cgroup CPU quota.
Kept 96 jobs for a consistent run; SMT occupancy is not equivalent to idle
physical cores. More concurrency would require separate throughput testing
because per-board 300-second limits can change outcomes.

Primary census `cargo test --workspace` exited 0: **2,555 passed, zero
failed, zero warnings**. Removed only the two temporary benchmark corpus
symlinks; their targets and the original checkout are untouched. The commit
contains only statistics.rs, this running log, and the measured results JSON.


### EPYC completed census replication and referee repeatability audit

The census-only fix is committed as `0217f96`. The new-server baseline and
census cohorts both finished all 845 attempts in `quality-epyc-full-01`.
Baseline has 840 scored cells, census 842. Matched results (same server,
96 jobs, same binaries/runtime, 300-second cap):

| Referee | Matched boards | U improved / regressed | Delta U | DRC gainers | Delta routing V | CPU ratio |
|---|---:|---:|---:|---:|---:|---:|
| KiCad | 739 | 5 / 5 | -708 | 1 | +3 | 0.99141 |
| Java DRC | 101 | 4 / 2 | -39 | 0 | 0 | 0.97008 |

No newly unscored outputs. Baseline failures include the four existing Java
failures plus `pcbench-teensy-weather-badge_teensyi2c`. Census newly scores
that PCBench board (0 U / 0 V) and issue756-tomu-fpga8 (11 U / 853 Java V);
neither is included in matched gains. Exports for both completed cohorts
were generated immediately and verified in workbench's metadata mirror.

All three additional routing violations are on nonSNES: one PA5 clearance
against the same U2 copper rectangle UUID previously investigated, plus two
Net-(P1-IRQ) shorts against U2 rectangle UUID
`76d0344a-c5da-4e58-b809-f59992a6205a`. This is consistent with the known
loss of copper-rectangle semantics in its DSN; the new shorts still count
as actual KiCad violations. Census leaves 121 U versus baseline 319 U on
this server. Five PCBench U regressions are ReSDMAC +1, decelerator4030 +1,
and Karabas revisions B +3, C +3, G +2. Further per-board inspection remains
required for the newly observed regressions; do not silently dismiss them
as deadlines. EPYC and workbench counts differ and are reported separately.

A new measurement limitation was reproduced, without modifying any routing:
24 paired census boards have different solder-mask error counts; 20 of
those pairs have byte-identical out.ses files. For badge2016, ran KiCad DRC
three more times on the **same saved baseline routed.kicad_pcb** with the
same project and CLI arguments. Mask error totals were **870, 869, 871**;
the original baseline referee recorded 868. Thus at least some mask-count
variation is downstream of routing/import, in repeated DRC itself. This
does not negate the geometrically proven nominal-clearance mask overlap,
and does not justify subtracting arbitrary counts from previous experiments.
Small mask changes need repeat scoring before causal attribution. Reports
are preserved locally in `/tmp/copperroute-quality/epyc-mask-repeat/` and
on the server in its backed-up `copperroute-logs/mask-repeat/`; paired SES
audit is `epyc-census-mask-ses-audit.json` locally.

Prepared cumulative census + zero-distance via guard helper at 0217f96.
`cargo test --workspace` passed **2,556 tests, zero failures/warnings**,
log `/tmp/copperroute-quality/workspace-census-via-progress.log`.
The cumulative three-fix helper previously passed 2,558 tests. Building
both Linux candidates sequentially with two build jobs on workbench;
full cumulative validation is still pending, so no via fix is committed.


Follow-up inspection: all five new EPYC KiCad U regressions have router
`final_state=TIMED_OUT` in both variants, with equal pass counts within
pairs: ReSDMAC 3, decelerator4030 2, Karabas C 5, G 4, B 5. CPU is
300.28–301.73 seconds. This supports a deadline-related hypothesis but is
not itself a matched longer-run reproduction. Original ReSDMAC is complete
with 1,965.2484 mm / 171 vias / 0 routing V; decelerator4030 is complete
with 28,322.1397 mm / 865 vias / 0 routing V. These remain useful original
boards for further inspection.

The partial projection cohort had one apparent new referee failure on
DoroidOscillo. Its referee log says `Unable to access the X Display` during
SES import; router output exists. Copied the entire cell to a separate
audit directory and rescored the unchanged SES under a fresh xvfb-run.
That succeeded: 4 U / 0 routing V / 48 mask errors, 101 vias, 1,777.7735 mm.
Original run cell remains intact. Thus this specific failure is referee
infrastructure, not a routing failure; final exports must explicitly
account for the rescoring rather than hiding the missing cell. Audit:
`/root/copperroute-logs/referee-retry/pcbench-DoroidOscillo-Board_Android_Oscilloscope/`.


Cumulative Linux source hashes verified against local test manifests; both
release builds succeeded. Shipped binaries and verified matching SHA-256:
`census-via-progress` d2083f8a8682aa88494e87272d80b51577e29d05337628ab85685187cbcc516a;
`census-via-guards` 49aec77e96d2e301e656da4c4414aad60204a72110a01aa7c608da9368c54b66.
Queued `/root/server-cumulative.sh` under a nonblocking flock. It waits for
the current full batch's completion marker, then runs parent census,
census + zero-distance, and census + both via guards on all 845 boards,
96 jobs / one routing thread / 300 seconds / ten passes. Run ID
`quality-epyc-cumulative-01`, config `/root/server-cumulative.toml`.
This supplies direct cumulative comparisons before any further commit.
Both existing workbench backup loops cover the new run's results/exports.


### Projection DRC trade: LPC2148 saved/autosave pair

The first two projection DRC gainers are duplicate versions of LPC2148
(saved and autosave), each 62 -> 1 U and 3 -> 8 routing V. All eight
candidate errors are track widths 0.2498 mm against KiCad's 0.254 mm
minimum. Candidate completes ten passes in 193.39 CPU seconds (saved
version); baseline times out in pass one at 300.18 seconds. Candidate
4,775.8046 mm / 103 vias versus original 5,513.9113 mm / 116 vias; the
original is complete with zero routing V (72 preexisting mask errors).

Opened original routed board with pcbnew: U1 pad 6 GND is centered at
(119.868, 59.578), size 1.0 x 0.25 mm. Designer escapes straight outward
on F.Cu with **0.3048 mm** track to (119.868, 57.54148). Router's offending
GND escape uses **0.2498 mm**. `Pin::get_trace_neckdown_halfwidth` computes
half the minimum pad width minus one internal unit, explaining this exact
width. DSN nominal width is 254 um; its pad is narrower than that, but the
original demonstrates the escape need not itself be narrower than the pad.
Previous nominal-clearance full experiment routes this same saved board
0 U / 0 routing V; standalone smoothing remains 62 U / 3 V. Thus the via
projection fix exposes existing unnecessary neckdown while letting routing
finish. Do not reject it solely because the two versions gain five V each;
also do not call it DRC neutral. No width patch made without reproducing
why the legal full-width escape was rejected.


LPC neckdown instrumentation (isolated `copperroute-quality-lpc-diagnostic`
helper, projection-only source plus environment-gated logging) reproduced
U1 GND pin ItemId(619), center (1198680,-595780). Nominal half-width 1270,
neck half-width 1249, clearance including margin 1794. It checks a segment
from (1197980,-587614) to (1197980,-595080), returns only 101 units of
full-width permission, then the final diagonal to the pad returns zero.
This locates the decision precisely; diagnostic routing is still running.
No diagnostic changes are on the primary branch or benchmark candidates.


Confirmed neighboring U1 pad 7 /VDDA at (119.368,59.578), size 1.0 x
0.25 mm, rotation 270 degrees, from the original board. Its right edge
is x=119.493 mm. The logged nominal-width GND segment is centered at
119.798 mm with half-width 0.127 mm: copper gap is **0.178 mm**.
DSN requires **0.1778 mm**, so the full-width segment has 0.0002 mm
clearance to spare. Insertion's margin-inclusive check requires **0.1794
mm**, rejecting it by 0.0014 mm. Necking to 0.2498 mm total width raises
the gap to 0.1801 mm and clears that artificial margin, at the cost of a
real minimum-width violation. This is the same measured maze/insertion
margin mismatch as the AVR regression, now tied to the projection fix's
DRC trade. It strengthens the case for nominal clearance, while leaving
its separately measured solder-mask trade and missing-output cases open.
No speculative blanket minimum-width clamp added.


Projection cohort finished 845 attempts, 840 scored before infrastructure
rescore. Matched KiCad 738: 6 U improvements / 3 regressions, -139 U,
2 DRC gainers / +10 V, CPU ratio 0.98134; Java 101: 3 / 3, -1 U,
0 DRC gainers / unchanged V, CPU ratio 1.00192. One newly unscored KiCad
cell is the documented display failure; its copied-cell retry succeeds.
Baseline teensy-weather-badge's failed import also succeeds when retried
in a separate audit directory: 0 U / 0 routing V, 2 invalid-outline errors.
These retries do not reroute and original cohort metrics remain preserved.
Final corrected comparisons must account for both repaired referee cells.


Applied the two proven successful referee retries to their completed
original cells after asserting identical SES bytes and saving the entire
pre-repair cells under `/root/copperroute-logs/pre-referee-repair/`.
Rebuilt metrics through `bench.metrics.build`, regenerated the four
completed exports, and saved provenance in `referee-repairs.json`.
Corrected baseline, zero-distance, and projection each have 841 scored
cells; census 842 (newly scores issue756-tomu-fpga8). All three candidate
comparisons now cover all 740 KiCad boards, with no new referee failures.
Net U/V and improvement/regression counts are unchanged. Exact corrected
CPU ratios and aggregates are in `docs/routing-quality-epyc-results.json`.

LPC diagnostic completed ten passes in 92.624 seconds. Archived its
instrumentation patch as `/tmp/copperroute-quality/lpc-neck-diagnostic.patch`
and removed the instrumentation from the helper source. The production
branch remains census-only, with new findings in the running log.


### Proba actual-clearance lead: independently reproduced by Rust DRC

Ran CopperRoute `drc` on the baseline Proba 300-second pilot's DSN plus
exported SES. Import succeeded: 816 wires, 302 vias, zero import errors.
Rust DRC reports GNDREF trace vs +3V3 trace clearance 0.1017 mm, and
GNDREF via vs +3V3 trace 0.0517 mm, both against explicit DSN 0.1500 mm.
These correspond to KiCad's 0.1016 / 0.0516 mm errors at via
(185.2479,98.2516) on the GND layer. Rust groups the affected polyline as
one trace, explaining two Rust reports versus three KiCad segment reports.
DSN GND and PWR classes both explicitly specify 200 um width / 150 um
clearance; all four layers are signal layers. Thus unlike Azalea, this is
not explained by an omitted local clearance rule. Need capture when the
violation first enters the board before proposing a fix. DRC command exit
1 is the expected detected-violation result, not a failed import. Artifacts:
`/tmp/copperroute-quality/proba-internal-drc.{json,log}`.


Proba first-pass isolation completed normally (144.275 seconds, max passes
one), already containing the target via/trace and trace/trace clearance
errors. Added environment-gated after-connection DRC in a separate
`copperroute-quality-proba-diagnostic` helper. It first detects the target
immediately after routing ItemId(628), net 12 GNDREF, in pass one. Report:
Via ItemId(6657) vs +3V3 Trace ItemId(35542), layer 1, expected 1500 internal
units / actual 516.985855. Requested a safe router stop after that complete
connection; captured SES/result/log in `/tmp/copperroute-quality/proba-first-violation*`.
Diagnostic run took 23.18 seconds. A second diagnostic checkpoint before
`opt_changed_area` is being built to distinguish insertion from subsequent
optimization. No production patch inferred from the DRC symptoms alone.


EPYC first batch completed at 2026-09-09 04:39:26 UTC; cumulative batch
started automatically at 04:39:30. Verified live wrapper 3599523, uv
3599538, and active census cells. All six first-batch exports verified in
workbench's metadata backup. Smoothing+empty-proposal final matched KiCad:
740 pairs, 13 improvements / 8 regressions, -25 U, one DRC gainer,
-14 routing V, CPU ratio 1.00684. Java: 101 pairs, 6 / 5, -12 U,
zero DRC gainers / unchanged V, CPU ratio 0.99792. No newly unscored
outputs. Five candidate experiments plus baseline are now complete on
EPYC. Exact results JSON updated separately from the workbench report.

Proba second diagnostic: the target clearance violation is already present
before opt_changed_area, involving via6657 and trace35535. The later
optimizer changes the trace identity to35542 while preserving the same
0.0517 mm gap. This excludes post-route optimization as the first source.
Added finer checkpoints around forced trace insertion and endpoint joins
in the isolated diagnostic helper; next run is underway.


Proba third diagnostic narrows first introduction further: immediately
after forced GNDREF trace insertion on layer 1, before endpoint connection
and before opt_changed_area. The inserted segment runs horizontally from
(1870653,-983017) to (1851623,-983017). Its shove leaves +3V3 at y=-979500,
3517 units from the new GND trace: that clears two 1000-unit half-widths
plus the 1500-unit rule. But the existing GND via is at y=-982516,
501 units above the new GND line, and therefore only 3016 units from the
shoved +3V3 line. Its 1500-unit radius plus the foreign trace's 1000-unit
half-width leave approximately 517 units clearance, below 1500. Need
capture the original foreign trace geometry to build a minimal shove
reproduction; do not patch based only on this geometric explanation.


Prepared primary PR branch step two by applying the reviewed zero-distance
via guard patch on top of 0217f96. Full primary workspace test gate is
running in `/tmp/copperroute-quality/workspace-primary-census-via-progress.log`.
No commit until cumulative corpus comparison is complete. Remote binary
remains immutable and source-equivalent to this branch step. The primary
now has two changed source/test files plus reporting updates; projection
and diagnostic instrumentation are not applied there.


User requested CONTRIBUTING.md compliance and the benchmark report pasted
into the PR. Read the file and fetched origin/main: it advanced from
636e039 to **6886640** (ratsnest viewer PR). Reviewed the Rust diff: existing
connectivity list/max-connection calculations are extracted into shared
functions, with viewer consumers added. Nonetheless, final PR comparison
will use that exact latest main, not substitute old-main measurements.
Primary old-base census+zero workspace gate passed **2,556 / 0 failures /
0 warnings**. Preserved work with a named stash, rebased onto origin/main,
and restored successfully. Census commit is now **84edea1** (formerly
0217f96); pending zero-distance source/test and all reporting files survived.
New primary workspace gate is running after the rebase.

Shipped main's incremental git bundle to workbench. Detached helpers at
6886640: `copperroute-quality-main-6886640`, `...-latest-via-progress`,
`...-latest-via-guards`. Applied previously verified cumulative source
archives to the latter two. Release builds use --locked and two build
jobs sequentially; no active candidate binaries changed. Final comparison
will use the requested imported 845-board corpus, including every selected
PCBench board, and paste actual `bench pr-summary` output into the PR.
CPU observations will be reported as single-repetition measurements, not
a timing-speedup claim (CONTRIBUTING requires three repetitions for that).

Proba before-shove geometry captured successfully: only two nearby layer-1
traces, GNDREF35531 and +3V3(net32)35530. Full line equations and after-shove
items are saved in `proba-stage4.log`. Archived all diagnostic source edits
as `/tmp/copperroute-quality/proba-diagnostic-reviewed.patch` and restored
the helper's three instrumented files; helper is clean. Further minimal
shove-test work is parked while finishing the PR's existing validated fixes.


Prepared final-run wrapper `/root/server-pr-final.sh` (not launched yet).
It waits for the previous cumulative batch and verified latest-binary
readiness, then runs latest main, zero-only cumulative, and both-via-guards
cumulative on the requested 845 boards. It generates the required
`bench compare --baseline main --against change --fail-on-regression`
and `bench pr-summary`, using CPU timing. A nonzero comparison gate is
saved as evidence, not suppressed as success. Reports and PR summary are
copied into the backed-up logs directory. Final config/commit identities
and binary readiness remain to be supplied after validation and builds.


Rebased primary census+zero `cargo test --workspace` completed: **2,563
passed, zero failed, zero warnings**, reflecting the newly landed main
tests. Log `/tmp/copperroute-quality/workspace-primary-latest-via-progress.log`.
Latest baseline release hash e3eb7b36df5afda3669892cbd363e370591af84ba312a2b2628c5303617491ff;
latest census+zero hash d5c6301b3e46ee066c77d2c4139d11812b523f5d1ba1c177888d962050a1f75f.
Both built --release --locked on workbench and are transferring to EPYC.

Cumulative zero-distance guard has completed all 740 KiCad scores: three
U improvements, four regressions, -25 U, zero DRC gainers / unchanged
routing V, CPU ratio 0.99621. Regressions: Blitz .C68 +9, Blitz Rev.K +1,
Karabas A +2, Karabas B +10. All are router TIMED_OUT; first three end in
the same pass in both variants, Karabas B ends pass4 versus parent pass5.
These losses remain counted, not excused by their deadline sensitivity.
Remaining Java cells must finish before commit numbers are finalized.


Cumulative batch `quality-epyc-cumulative-01` is complete (845 attempts per
candidate). Zero-distance guard versus cache parent, on 841 common scored
boards: KiCad 740, 3 improved / 4 regressed, -25 unrouted, no DRC gainers,
0 routing violation change, CPU ratio 0.99621; Java 101, 3 improved /
3 regressed, -14 unrouted, no DRC gainers, 0 violation change, CPU ratio
0.99855. No newly unscored boards. Aggregate -39 unrouted / unchanged DRC.
The four KiCad regressions and deadline observations are recorded above.
These measured old-main cumulative results support landing the zero-distance
guard separately; exact latest-main validation remains required for the PR.
Latest-main workspace gate: 2,563 passed, no failures or warnings.

Both guards versus cache parent also completed: KiCad 740, 10 improved /
6 regressed, -162 unrouted, 2 DRC gainers / +10 routing violations, CPU
ratio 0.96534; Java 101, 4 improved / 3 regressed, -20 unrouted, no DRC
gainers / unchanged violations, CPU ratio 0.99056. The two LPC2148 copies
account for the +10 width violations; their insertion-margin/neckdown root
cause is documented above. Incremental comparison against zero guard and
remaining regressing-board inspection precede the projection commit.


Committed zero-distance guard as **26595d8** after its latest-main workspace
gate. Applied the reviewed projection patch on top; combined production
via source SHA256 is 62fab1ae785257ff7695751eeba0ecd32a5c40f667a141c84a0d42fcd09f0b8a,
matching the built cumulative and latest-main helpers. Primary full workspace
test is running in `workspace-primary-latest-both-guards.log`.

Incremental projection comparison versus cache+zero: KiCad 740, 10 improved /
3 regressed, -137 U, 2 DRC gainers, +10 routing V, CPU ratio 0.96901;
Java 101, 2 improved / 1 regressed, -6 U, zero gainers / unchanged V,
CPU ratio 0.99199. Two newly clean boards (Hardware Playground serial gateway
and Starling); zero clean losses. No newly unscored boards. Newly scored
Java fixture is recorded separately in `routing-quality-cumulative-results.json`.

Reviewed every incremental U regression's saved metrics: Blitz Rev.K
331 -> 344 and Blitz copy 330 -> 345, both variants TIMED_OUT in pass1
at approximately 303 CPU seconds; Karabas revG 79 -> 81, both TIMED_OUT
in pass4 at approximately 301 CPU seconds. Java issue420 214 -> 215
referee U, both self-report 377 U and TIMED_OUT at approximately 303 CPU
seconds. These measurements establish deadline-limited outputs, not proof
that longer runs eliminate the losses. All remain in the comparison.
Blitz originals are complete and DRC-clean, 33,679.8394 mm / 527 vias
versus approximately 14,500-15,043 mm / 677-681 vias in our incomplete
outputs. Karabas original is complete with no routing violations,
10,188.6705 mm / 776 vias, versus approximately 7,468-7,598 mm / 314-315
vias in our incomplete outputs. Partial wirelength cannot establish an
efficiency improvement. LPC2148's real width and mask trade is retained.

Final exact-main corpus batch `quality-pr-main-6886640` launched on EPYC,
bench PID1195986. Main=6886640; zero-only=26595d8; change is explicitly
labelled 26595d8-projection-62fab1ae until the tested projection source is
committed. All three immutable binary hashes and source provenance are
saved in `pr-final-source-provenance.json` on laptop and backed-up server
logs. Configuration: 845 boards, 300 seconds, ten passes, one thread,
96 jobs. Referee environment comes from the previously verified server
wrapper. Both artifact and metadata backups to workbench remain active.


Primary latest-main cache+both-guards `cargo test --workspace` passed:
**2,565 tests, zero failures, zero warnings**, process exit0. Existing
JVM recordings remain unchanged. Landing projection as a separate commit
with the explicit cumulative -143 U / +10 routing V trade, two new clean
boards and no clean losses. The two duplicate LPC2148 boards gain five
width violations apiece while losing 61 unrouted connections each. This
is an intentional progress correction backed by the reproduced oscillation,
not a claim that its additional routing is DRC-clean. Latest-main full
comparison remains running and will determine the final PR recommendation.


### Workbench standalone zero guard: contradictory repeat, retained

`quality-via-progress-full` completed all 845 attempts. Before referee repair,
KiCad 732 matched scores: 3 improved / 11 regressed, +172 U, 1 DRC gainer /
-7 V, CPU ratio 1.02143. Java 101: 5 improved / 8 regressed, +21 U, no
DRC gainers / unchanged V, CPU ratio 1.04713. Eight newly unscored KiCad
outputs: seven SES-import failures and Djinn's missing output. This contradicts
the positive EPYC standalone and cumulative measurements and remains part
of the evidence; no favorable subset substitutes for it.

Every U-regressing output is TIMED_OUT. Several large KiCad losses
(Karabas revisions A/B/C/G, 40-channel switch, Dropbot panel) received
only about 193-209 CPU seconds, versus about 299-301 in the old baseline.
This establishes a CPU-budget confound under the wall-time deadline, not
proof that contention accounts for every loss. Djinn hit the external
360-second limit and produced no SES; its recorded CPU/RSS zeros are
missing measurements, not zero actual resource consumption.

Preserved all seven failed-import cells under workbench
`~/copperroute-server-backup/workbench-via-referee-repairs*/original/`.
The first diagnostic retry omitted the explicit KiCad CLI variable,
failed tool discovery, and changed no benchmark cells. The second retry
uses the explicit installed CLI and a fresh X display per board. Six of
seven have now succeeded on byte-identical saved SES, with original
metrics/referee artifacts retained before rebuilding metrics. Final repaired
counts must replace the preliminary numbers above in the summary tables.

Started `quality-via-progress-regression-repeat` on workbench, bench PID85433:
13 KiCad boards (every U regressor or DRC gainer, plus missing-output Djinn),
both original main636 binary and unchanged zero-distance binary, 300 seconds,
ten passes, one routing thread, jobs12. No concurrent release builds.
This targeted matched repeat investigates the observed losses; it will
not replace the completed full run. Final latest-main EPYC batch remains
active and has started its combined candidate.

Clarified clean-pass terminology: the two projection clean gains mean
zero U and zero routing V. Starling has zero errors of all KiCad types;
Hardware Playground serial gateway has nine courtyard/outline errors
(six pth_inside_courtyard, three invalid_outline).


All seven workbench import retries succeeded; unchanged SES hashes and
original failures are recorded in `workbench-via-referee-repairs.json`
on the laptop and workbench backup. Re-exported the run. Final matched
KiCad 739: 3 improved / 12 regressed, +196 U, 2 DRC gainers / -6 V,
CPU ratio 1.01658. Java unchanged at +21 U / 0 V / CPU 1.04713.
Djinn is the sole newly unscored board. Referee repair adds SRAM-bank
108 -> 132 U and 0 -> 1 V to the investigated regression set.


Final PR checks: fetched origin/main again; it remains 6886640, the
benchmarked baseline. Whole-workspace formatting check flags pre-existing
formatting in unchanged copper-dsn/tests/kicad_browser_geometry.rs and
copper-web/src/lib.rs, plus one extra blank line in the new projection
test module. Removed only that new blank line; all three changed Rust
files now pass rustfmt --check. A final workspace test is running in
`workspace-primary-final-format.log`. Production code is byte-identical
to the benchmarked source; the sole new source-file difference is test
whitespace. No unrelated baseline formatting changed.

Latest-main candidate `change` has an infrastructure referee failure on
HellScribe: original log says 'Unable to access the X Display'. Copied
the complete cell to `/root/copperroute-logs/pr-final-referee-retry/`,
rescored byte-identical SES with a fresh X display: 0 U / 0 routing V /
0 all-type errors, 21 vias, 1,567.7648 mm. Original run is still active;
repair application and comparison regeneration wait for its completion.
This will be recorded as referee repair, not rerouting.


Final post-format workspace suite completed with exit0: **2,565 passed,
zero failures, zero warnings**. No production changes after the measured
build; only removal of a blank line in the projection test module.


### Final-run decelerator4030 clearance gainer

Inspected main/zero/combined saved KiCad DRCs. Main and cache+zero each
have 12 clearance errors; combined has 14 (+2 net). Additional combined
pairs include TCK tracks on Dolna against GND via (160.3862,42.7482):
actual gaps 0.1774 / 0.1304 mm against required 0.2 mm. Another new pair
is _RMC against +5V via (87.6838,78.2013), gap 0.1565 mm. Some earlier
ARM13 pairs disappear, so raw new-pair count differs from net +2.

DSN explicitly specifies default clearance 100 um and Power class
(+3.3V,+5V,GND) clearance 200 um, width 381 um and 900:500 via rule.
The SES includes a 550:250 GND via at the offending point. The original
board is complete, 0 routing errors / 2 outline errors, 28,322.1397 mm
and 865 vias. This is not evidence that KiCad's 0.2 mm requirement was
omitted from the DSN.

Imported combined SES into CopperRoute's own DRC: 1,708 wires / 566 vias,
zero import errors. Checker reports 12 clearance item-pair errors,
including GND-via/TCK at expected 0.2000 mm / actual 0.1303 mm, plus
267 dangling vias and one dangling trace. KiCad counts individual track
segments, so its 14 clearance reports are not directly comparable to
our polyline-pair count. The rule exists and the checker recognizes the
violation. Whether clearance is lost during routing or geometry changes
later requires an insertion-level reproduction; no speculative patch
was added. The +2 KiCad change remains a real regression in the report.
Artifacts: `decelerator-internal-drc.json` and `.log`, copied to laptop
and automatically backed up from server logs.


### Final exact-main full corpus complete

`quality-pr-main-6886640` completed all three 845-attempt cohorts. Applied
HellScribe referee repair only after completion, preserved the original
cell and original comparison/summary in `pr-final-pre-referee-repair`,
then rebuilt metrics, re-exported and reran compare/pr-summary.

Combined versus latest main, matched KiCad740: 11 U improvements /
4 regressions, -882 U, 4 DRC gainers / +15 routing V, CPU ratio 0.95025.
Java101: 5 improvements / 3 regressions, -53 U, no DRC gainers / -1 V,
CPU ratio 0.95918. Aggregate -935 U / +14 V. Two routing-clean gains,
no clean losses. KiCad solder-mask +151 / all-type errors +166.
No newly unscored boards. issue756-tomu-fpga8 newly scores 11 U / 853 V
and is excluded from matched improvement totals.

`bench compare --fail-on-regression` exits1, reporting eight quality
losses (its score-based category is broader than U regression). Its
comparison includes failure cells as well as successfully scored boards;
our referee-based matched quality table uses 841 boards. Do not present
the failed gate as passing or substitute aggregate gains for its output.
Full unabridged `bench pr-summary` is 83 KB, mostly repeated single-sample
noise caveats, saved as `routing-quality-pr-performance.md`.

Workbench 13-board matched repeat completed. Baseline Djinn lacks SES,
zero guard newly scores it at 393 U / 0 V. On 12 common scored boards:
5 U improvements / 2 regressions, +7 U, one DRC gainer / +5 V, CPU
ratio 0.98790. The two U losses are ReSDMAC 128 -> 145 and decelerator
412 -> 483, both around 300 CPU seconds. Chess improves 82 -> 76 U
but gains five V (309.37 -> 268.79 CPU seconds). Thus the original
full-run negative result is not explained entirely by contention. The
focused repeat reduces the broad regression pattern but retains real
losses on two deadline-limited designs. The separately queued SRAM-bank
pair is active. These repeats remain supplementary to the full corpus.


User requested whether the temporary EPYC server can be powered off.
Verified no active copperroute or bench processes. Final workbench
rsync dry-run comparison reports zero differences for detailed results
and logs; checksum comparison reports zero export differences. Laptop
also has final exports, matched results and full performance report.
Stopped workbench backup-loop PIDs4179845/4181848 after verification.
Server is released: no further work depends on it being available.
Remaining focused repeats run on workbench.


Final SRAM-bank matched repeat `quality-via-progress-sram-repeat` completed:
both main636 and standalone zero guard finish ten passes with 108 KiCad
unrouted / 0 routing violations, 171 vias and 4,339.721 mm self wirelength.
CPU 180.91 versus 179.61 seconds. The original full-run 108 -> 132 U /
0 -> 1 V regression does not reproduce in this matched completed run.
This is supplementary evidence; the original full run remains recorded.
No routing runs or required validation remain pending. The proposed
combined changes retain their measured trade-offs for case-by-case review.

## 2026-09-08: follow-up after PR #21, KiCad-only validation

Baseline: merged main `e9d10c21`, isolated worktree `copperroute-mask-quality`,
branch `routing-quality-kicad`. The user requested removing Java DRC scoring and
revisiting nominal clearance + smoothing + the now-merged connection-check cache,
then exploring larger structural algorithm changes on workbench.

### KiCad-only benchmark scoring (tooling change)

Removed the Java DRC execution module, Java referee configuration and the Java-based
`corpus connections` command. Java can still be compared as a router candidate.
Default scored runs select only KiCad boards; explicit DSN-only scoring and legacy
DSN rescoring fail before routing or modifying results. `--no-referee` remains
available for DSN routing diagnostics. Historical reports remain readable.

Failing-first tests reproduced three unwanted behaviors: automatic selection of a
Java-only fixture, acceptance of explicit DSN scoring, and mutation of legacy
results during rescoring. All three passed after the change. Adapting the existing
interrupted-rescore test to KiCad uncovered that comparisons only guarded Java
measurements during incomplete rescoring; the guard now excludes all measurements
from such runs until a full rescore completes.

### New main baseline (in progress)

Workbench built the exact merged-main release binary in `~/copperroute-kicad-main`.
Copied the previously missing 11 usable local KiCad fixtures to its durable corpus.
Run `quality-kicad-main-e9d10c2` selects **751 KiCad boards (740 PCBench + 11 KiCad
fixtures)**, with 10 passes, 300-second timeout, one router thread and 12 jobs.
KiCad CLI, KiCad Python and GNU time are explicitly configured. No Java referee
is involved. This differs from the old 845-board population (740 KiCad + 105 DSN),
so future totals must not be compared directly with old mixed-referee totals.

### Nominal-clearance mask investigation (no fix implemented yet)

The prior LPC2148 audit identified a real missing constraint: its global mask
expansion is 0.2 mm while DSN copper clearance is 0.1778 mm. Nominal routing places
a foreign-net track 0.18058096 mm from P1 pad 38: copper-legal, but its mask aperture
exposes that track. The original designer leaves at least 1.016 mm at this location.
The original already has 72 mask errors; the previous main/nominal outputs had
130/199. The native KiCad DTO and DSN input both currently omit mask metadata.
Investigating per-pad outer-layer constraints rather than a blanket clearance
increase. KiCad's own mask checker and documentation confirm that aperture-to-
foreign-copper clearance is a separate constraint:
https://docs.kicad.org/doxygen/drc__test__provider__solder__mask_8cpp_source.html

Tooling validation so far: `uv run pytest -q` passed **257 tests**, with one
integration test skipped because this fresh worktree has no imported KiCad inputs.
A real workbench smoke check found 24 scored cells, all referee status `ok`, and
nonzero violations on two of them. The full Rust workspace check is still running.

KiCad 10.0.3 `pcbnew` inspection of the original LPC board confirms 331 pads at
0.2 mm expansion: 204 front-only, 122 both sides, five back-only. Any constraint
import must preserve those layer distinctions. KiCad's checker separately uses
mask-to-copper clearance for aperture/copper tests and web width for aperture/
aperture tests; importing only a board-wide copper floor would conflate them.

Structural research leads for the next stage (not yet implemented or measured):
- McMurchie and Ebeling, PathFinder: history-dependent negotiated congestion.
  https://janders.eecg.utoronto.ca/1387_2015/readings/pathfinder.pdf
- Kahng et al., TritonRoute: detailed routing with search and repair.
  https://vlsicad.ucsd.edu/Publications/Journals/j133.pdf
CopperRoute already carries per-item rip-up costs and board history, so first
check whether repeated congestion survives item replacement and whether repair
actually targets the DRC-producing geometry before proposing a new mechanism.

Final tooling validation: Rust workspace **2565 passed, zero failed** (`cargo test --workspace`); benchmark **257 passed, one skipped**.
No router behavior changed in the KiCad-only scoring commit; the running corpus
baseline is for the subsequent routing experiments.

### LPC2148 pad-mask pilot 01 (held; not a production fix)

A failing-first geometry test proved that a foreign-net copper shape outside the
old clearance remained accepted on an exposed pad layer. The prototype gives the
pad a derived clearance class, preserves wider existing pair rules, and changes
only the supplied layers. It reuses equivalent classes. The test then passed;
same-net copper and the unexposed layer remain permitted. The smoothing reserve
test was also replayed against main, failed at a one-unit available move, then
passed after restoring the held smoothing change.

Temporary env-gated load instrumentation reads pad floors extracted with KiCad
10.0.3 `pcbnew`. **This ingestion is diagnostic only, with unwraps and incomplete
metadata matching; it must be removed or replaced before committing.** It matched
323/331 LPC pads; the eight omitted DSN pads are SW1/SW3/SW4/SW5, pins 1 and 2.
The resulting board uses seven clearance classes (no per-pad matrix explosion).

Local, one thread, 10 passes, 300-second timeout, same binary with/without the
pad constraint, KiCad referee, same original stripped/project input:

| Candidate | Unrouted | Routing DRC | Mask DRC | All errors | CPU s | RSS MiB | Vias | Length mm |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| Nominal + smoothing + merged main fixes | 0 | 0 | 199 | 199 | 77.94 | 74.9 | 101 | 4887.8459 |
| Above + pad-mask floors | 1 | 11 | 68 | 79 | 87.48 | 74.5 | 104 | 4903.4953 |

One board regressed connectivity (+1); one gained routing violations (+11);
mask errors fell by 131, all errors by 120; CPU ratio 1.1224. This is **not yet
mergeable**. All 11 routing errors are 0.2498 mm neckdown segments below the
board's 0.254 mm minimum, on four nets near U1. The remaining connection is
U1.36 `/u_led_1`. The original has 72 mask errors and zero routing errors.

A separate diagnostic copy widened those 11 segments to 0.254 mm and reran
KiCad DRC: zero routing errors, 72 mask errors. Thus the width repair creates
four mask errors; indiscriminate widening trades one violation type for another.
It is evidence for investigating full-width pad escapes, not an accepted fix.
Artifacts: `/tmp/quality-kicad-mask-pilot/`, including raw per-cell metrics,
KiCad reports, extracted pad data, prototype patch, and the widened diagnostic.

Workbench baseline routing completed all 751 attempts. Eight GTK SES-import
failures are being rescored from unchanged sessions with a fresh X display for
each cell. First rescore script used relative result paths (invalid under the
referee's cell working directory); corrected to absolute paths before retrying.
The initial referee reports/logs/metrics are preserved in each cell.

### Exact-main KiCad baseline completed

Run `quality-kicad-main-e9d10c2`: **751/751 referee status `ok`** after rescoring
eight GTK import failures with fresh displays and unchanged SES files. Export
and detailed metrics/referee JSON copied to `/tmp/quality-kicad-mask-pilot/`.

| Population | Boards | Unrouted | Routing DRC | CPU seconds | Mean / max RSS MiB | Connected | Routing-clean and connected |
|---|---:|---:|---:|---:|---:|---:|---:|
| PCBench, KiCad | 740 | 5321 | 947 | 37427.84 | 23.36 / 453.1 | 520 | 480 |
| Local KiCad fixtures | 11 | 243 | 59 | 714.45 | 26.91 / 75.2 | 6 | 5 |
| Total | 751 | 5564 | 1006 | 38142.29 | — | 526 | 485 |

Routing DRC excludes mask/artwork; detailed referee files preserve mask and all
error counts, which must also be compared for the mask experiment. These totals
must not be interpreted as a gain against the old mixed Java/KiCad totals.

Next full run launched: `quality-kicad-nominal-01`, nominal + smoothing on exact
merged main (therefore includes connection-check cache and both via optimizer
guards). Separate workbench helper `~/copperroute-kicad-nominal`, only the two
reviewed nominal/smoothing source files changed. Same 751 boards, jobs12,
threads1, passes10, timeout300. Binary and source-patch SHA256 recorded on
workbench at `/tmp/quality-kicad-nominal-sha256.txt`. This run does not include
the temporary pad-mask prototype.

The eight unmatched entries in LPC pilot01 were duplicate physical switch pads:
KiCad emits names such as `1@1` in DSN. Pilot02 switches to component + physical
position matching, retaining distinct front/back exposure per pad. It is running
locally; no quality result claimed yet.

### Follow-up measurements and full-width pad escape

The previous goal turn made progress: completed the KiCad-only tooling commit,
two full corpus measurements, four local routing pilots and a failing-first
full-width escape test. No new routing changes have been committed.

Full nominal + smoothing run `quality-kicad-nominal-01` completed with all 751
KiCad referee statuses `ok` (no rescore needed):

| Population | U improved / regressed boards | Unrouted change | Routing DRC gainers | Routing DRC change | Mask change (gainers) | CPU ratio |
|---|---:|---:|---:|---:|---:|---:|
| 740 PCBench | 126 / 50 | -463 | 30 | -33 | +240 (44) | 0.833036 |
| 11 KiCad fixtures | 3 / 0 | -17 | 1 | -2 | 0 (0) | 0.972496 |

Total unrouted 5564 -> 5084; routing DRC 1006 -> 971; mask DRC 13834 -> 14074;
all KiCad errors 17978 -> 18183. Fully connected 526 -> 575; routing-clean and
connected 485 -> 528. CPU 38142.29 -> 31873.55 seconds. Held for the mask regression.
Detailed metrics and exports are copied locally beside the baseline artifacts.

LPC pilots after pilot01 (all use nominal + smoothing and mask floors):

| Pilot | Pad mapping | Unrouted | Routing DRC | Mask DRC | CPU s | RSS MiB |
|---|---|---:|---:|---:|---:|---:|
| 02 | Position, 329/331 | 0 | 5 | 69 | 85.40 | 73.2 |
| 03 | Unique names or position, 331/331 | 0 | 12 | 68 | 79.58 | 76.4 |
| 04 | Complete mapping + full-width dogleg | 1 | 0 | 68 | 83.30 | 71.7 |

Two C5 pads (45-degree footprint) were not matched by the position-only tolerance;
unique names resolve them. Duplicate switch pads use their positions. This remains
prototype ingestion, not yet a general importer.

The dogleg test uses the actual LPC pad-corner geometry translated to the origin:
0.254 mm track, 0.2 mm pad clearance, pads 0.25 x 1 mm at 0.5 mm pitch. The direct
diagonal is blocked but both segments of an L-shaped route are clear. Existing
insertion produced a 0.2498 mm neck (failing test). Trying the clear full-width
dogleg first passes, with zero internal clearance errors. Failed proposals stay
on a cloned board; a successful proposal invalidates the engine's room and drill
caches. This addition retains the existing neckdown fallback.

Pilot04's remaining connection is **VDDA, U1.7**, not the LED net from pilot01.
Original inspection: VDDA escapes vertically from (119.368,59.578) to
(119.368,58.027); GND at the neighboring pad stays vertical to y=57.5415.
Our GND turns left at y=58.8364, reaching x=119.319 and crossing VDDA's outward
path. This motivates testing pad-exit planning. A diagnostic sweep of the existing
pin-edge-to-turn setting at 0.5 and 1 mm is running locally to determine whether
its correction mechanism can recover that access; it is not a proposed global
clearance or routing-rule change.

Workbench is extracting per-pad mask metadata for all 751 references with KiCad,
including project mask-to-copper clearance and each pad's front/back exposure.
This will permit a full-corpus mask-constraint pilot, independently of the dogleg.

### Mask corpus launch and rejected global exit distances

All 751 pad metadata files were extracted successfully. Full workbench run
`quality-kicad-mask-01` is active, candidate `nominal-smoothing-mask`, using
nominal + smoothing + pad-mask floors, without the dogleg. Source patch and
binary hashes are retained on workbench. The status check found 53 completed
metrics; no partial-corpus quality conclusion is drawn.

Local pilot05 completed both global pin-edge-to-turn diagnostics:

| Exit distance | Unrouted | Routing DRC | Mask DRC | CPU s | RSS MiB |
|---|---:|---:|---:|---:|---:|
| Existing setting + dogleg (pilot04) | 1 | 0 | 68 | 83.30 | 71.7 |
| 0.5 mm | 3 | 0 | 68 | 85.24 | 73.2 |
| 1 mm | 102 | 1 | 68 | 294.72 | 14.8 |

The 1 mm experiment exhausted the internal 300 second limit after only one pass
(`self.final_state=TIMED_OUT`; benchmark `timed_out=false` measures the external
process limit). Both settings are rejected; simply increasing the global exit
distance does not recover the blocked VDDA escape. Removed the temporary
`COPPERROUTE_PIN_EXIT_UM` hook locally after preserving the results. This does
not alter the frozen workbench candidate, where the variable is unset.

A failing-first pad-clearance test exposed an unnecessary class reassignment
when a named class already satisfied the requested floor but matched an earlier
class. Added an early no-op guard preserving its identity and the entire matrix.
All three tests in `cargo test -p copper-board --test pad_clearance` pass.
This guard is a newer local revision, not part of the running mask01 candidate.

Local pilot06 is running with two candidates on the same rebuilt binary: the
mask + dogleg control with the no-op guard, and the same routing with `/VDDA`
stably promoted ahead of other nets in each pass. A temporary
`COPPERROUTE_FIRST_NET` diagnostic hook performs that promotion; it is not a
proposed board-name or net-name special case. This isolates whether ordering
can preserve access without imposing a larger exit distance on every pad.
Artifacts and binary SHA are recorded under `lpc-mask-pilot-06`.

Structural investigation: `pass_runner.rs` creates a new `ripped_item_costs`
map for every connection attempt. The locator fills it from the selected path,
but the pass runner discards it afterward. Maze obstacle costs instead use the
pass-wide `start_ripup_costs * pass_no`, widths, detour and fanout factors, plus
randomization on selected passes. Thus this map is not persistent congestion
history. A spatial or net-pair history prototype would need to survive removal
and replacement of trace IDs; merely retaining this item-ID map would not
implement the congestion-history mechanism described by PathFinder. This is a
code finding and research lead, not a measured routing improvement.

### Ordering recovers the LPC escape; mapping audit

Pilot06 finished with KiCad status `ok` for both candidates:

| Candidate | Unrouted | Routing DRC | Mask DRC | CPU s | RSS MiB | Vias | Wirelength mm |
|---|---:|---:|---:|---:|---:|---:|---:|
| Mask + dogleg control, no-op guard | 1 | 0 | 68 | 84.52 | 77.3 | 99 | 4773.1164 |
| Same, VDDA first | 0 | 0 | 68 | 91.38 | 76.8 | 106 | 5079.4778 |

The updated control exactly reproduces pilot04's connectivity, DRC, vias and
wirelength. The diagnostic priority recovers the connection, supporting the
pad-access/order hypothesis, at +8.12% CPU, +7 vias and +306.3614 mm wirelength.
It is not a mergeable named-net policy. Saved its source patch as
`/tmp/quality-kicad-mask-pilot/pilot06-source.patch`, then removed the named-net
hook. Pilot07 will compare the control against stably prioritizing previously
failed items in later passes (`COPPERROUTE_FAILED_FIRST` diagnostic).

An audit of the in-progress mask corpus found positive unmatched pad constraints
on 35 boards at that snapshot, including unnamed mounting pads and duplicate pad
numbers. One concrete mapping issue is export rounding: Electronics MainBoard
U1 has four pads numbered 9. Its DSN places U1 at x=135736 um, whereas original
pad coordinates imply x=135735.529 um; y similarly differs by 0.495 um. The
prototype's two-board-unit position tolerance is only 0.2 um for this DSN.
Unique pad numbers use the name fallback, but duplicate-number pads miss it.
This is an ingestion limitation of mask01; the frozen full run is unchanged.
Audit artifacts: `mapping-audit-partial.json`, `electronics-in.dsn`,
`electronics-pads.json` under the local pilot directory.

### General failed-first pilot and internal mask DRC

Pilot07 completed, KiCad status `ok` on both candidates:

| Candidate | Unrouted | Routing DRC | Mask DRC | CPU s | RSS MiB | Vias | Wirelength mm |
|---|---:|---:|---:|---:|---:|---:|---:|
| Mask + dogleg control | 1 | 0 | 68 | 86.24 | 76.8 | 99 | 4773.1164 |
| Same, previously failed items first | 0 | 0 | 68 | 87.18 | 84.4 | 95 | 4940.6984 |

The general policy recovers the missing connection at +1.09% measured CPU,
-4 vias and +167.582 mm wirelength. No board-name or net-name special case.
Full workbench run `quality-kicad-retry-01`, candidate
`nominal-mask-dogleg-retry`, is queued behind mask01. Helper
`~/copperroute-kicad-retry` has six explicitly copied files from local pilot07:
nominal, smoothing, mask floors with the no-op guard, full-width dogleg, failed
item priority, and the temporary metadata loader. Its driver PID285526 waits
for mask01 driver PID261550 before building or routing. At the verified queue
check mask01 had 299 completed metrics. No internal mask-check changes or
rounding-tolerance change are included in the queued candidate.

The user's internal-DRC suggestion exposed a confirmed omission: the checker
has no solder-mask violation type, and Pin has no mask expansion metadata.
Added a first model field and a failing-first test for a pad aperture crossing
a foreign-net track despite legal copper clearance. Red test returned zero
violations; the first pad-to-routing checker now returns the expected one and
does not flag the same-net front trace or foreign-net back trace.

Generated the same geometry in KiCad 10.0.3 and ran its actual DRC: exactly one
`solder_mask_bridge`, zero `clearance` violations. Committed-fixture inputs and
reference report are staged as untracked files under
`crates/copper-drc/tests/data/solder-mask-pad-track/`; no git commit yet.
This first check does not yet implement mask-to-copper rule distances, aperture
web checks, arbitrary mask artwork, metadata import, or net-tie exceptions.
It is an incomplete implementation step, not a claim of KiCad parity.

The full `copper-drc` suite for the first mask check passed 127 tests, with one
ignored, including the existing KiCad oracle and JVM recordings. Log:
`/tmp/quality-kicad-mask-drc-suite.log`.

Extended the checker with the project `solder_mask_to_copper_clearance` rule,
including preserving a specified zero during constraint merging. A second
failing-first test places a foreign trace outside the aperture but inside its
required clearance: 0.23 mm copper gap, 0.2 mm expansion, 0.05 mm mask-to-copper
rule. The checker initially returned zero violations; it now reports one.
The first test also exposed a zero diagnostic shortfall for an actual overlap;
reporting combined copper-distance requirement versus measured copper gap
gives a useful shortfall for these positive-expansion cases.

KiCad 10.0.3 confirms the second geometry has exactly one mask bridge and no
copper-clearance error. Reference board, project and DRC report are under
`crates/copper-drc/tests/data/solder-mask-rule/`. The Rust test imports the
actual reference project, checks the converted rule, and checks the clearance
shortfall within one board unit, matching the existing integer-shape/bisection
measurement precision. Both targeted mask tests pass. Negative expansion
diagnostics, aperture web checks, net ties, importer support and corpus scoring
with the internal checker remain outstanding; no claim of full mask parity.

### Native KiCad mask metadata import

Added optional per-pad `solderMaskExpansion` data to the native KiCad JSON DTO,
keyed by absolute `F.Mask` / `B.Mask` and expressed in the input board unit.
The reader preserves it on each Pin independently of shared copper padstacks,
converts units, and rejects unknown mask layers and unrepresentable distances.
The failing-first import test initially read empty maps; it now verifies front
and back expansions, including -0.05 mm, on a four-layer board. All 15 tests in
`kicad_padstacks` pass.

The browser KiCad adapter now resolves pad, footprint and board mask margins,
preserves explicit zero rather than inheriting, expands `*.Mask`, and records
only exposed mask sides. Verified with the installed KiCad API that a pad's
explicit zero overrides a 0.15 mm footprint margin; negative margins also
remain negative. Two failing-first adapter tests cover those semantics; all
28 adapter tests pass. This native import path feeds the internal checker;
the frozen workbench DSN candidates still use the earlier sidecar prototype.
Routing enforcement from the native metadata is not wired yet.

### Routing enforcement and independent checker branch

Added native mask enforcement at the existing pipeline rule-minimum hook.
The hook runs both during preparation and at routing entry, so a project rule
attached after initial loading still raises the pad clearance. It uses the
same per-layer floor mechanism as the measured prototype and preserves higher
existing copper clearances. A failing-first routing test initially observed
1500 units instead of the required 5000; it now routes the connection, applies
5000 on the exposed front and retains 1500 on the back, with no internal DRC
violations. Log: `/tmp/quality-kicad-mask-routing-{red,green}.log`.

The user explicitly requested landing checker accuracy improvements independently
of routing outcomes. Created worktree `../copperroute-mask-drc`, branch
`fix/kicad-mask-drc`, based on KiCad-only tooling commit7243118. It contains
only the mask model/checker, native JSON/browser metadata import and tests;
it excludes nominal clearance, smoothing, dogleg, failed-first and routing
clearance enforcement. Full workspace tests are running there using the shared
local target directory to avoid another large build tree, log
`/tmp/quality-kicad-mask-drc-workspace.log`. Further checker edits should be made
in that worktree and integrated back explicitly, to keep experiment sources
attributable. No checker commit or PR yet: broader agreement with KiCad still
needs measurement.

At the latest live workbench check, mask01 had 490 completed metrics; retry01
driver285526 still waited for mask01 driver261550. Frozen sources and metadata
for both experiments remain unchanged.

### First finished-route mask comparison

The independent checker worktree passed `cargo test --workspace`: **2568 passed,
0 failed, 77 ignored**. The additional offline example test was added afterward
and passes separately; a final complete suite is still required before commit.

Added `copper-drc/examples/check_mask_session.rs` in the checker worktree. It
reads existing DSN/SES files, imports aperture metadata from the original board,
and checks the finished route without rerouting. A failing-first example test
reproduces Electronics MainBoard's duplicate-pad rounding miss with the old
0.2 um tolerance; 1 um matches the intended physical pad while rejecting another
component. This is diagnostic import code, not the frozen routing sidecar.

Extracted fresh LPC mask metadata preserving nullable exposure and negative/zero
expansions (`lpc-apertures.json`). Both finished-route audits matched all 331
pads with no unmatched router pins:

| Finished LPC route | Internal mask reports | KiCad routing-related mask reports | KiCad total mask reports |
|---|---:|---:|---:|
| Nominal + smoothing (pilot01) | 113 | 143 | 199 |
| Mask + dogleg control (pilot06) | 0 | 0 | 68 |

An initial comparison script missed KiCad's capitalized `Pad ... on F.Cu`
descriptions. Correcting that parser gives **107/113 internal reports matching
a KiCad pad/foreign-net/layer key**, with six unmatched reports to investigate.
This is a coarse pair comparison, not yet a segment-level precision/recall
claim. Differing segmentation and mask-aperture grouping can affect counts.
Artifacts: `lpc-{nominal,control}-internal-mask.json`, `lpc-pair-audit.json`
under the local pilot directory. No checker landing decision yet.

### Isolated KiCad checks resolve the first six disagreements

Created six derivative KiCad boards retaining the questioned pad and the
foreign-net routing, and ran KiCad 10.0.3 DRC on each. C10.2, C11.1, P3.39 and
SW1.1 produce mask errors in isolation; U1.1 and U1.39 do not. The four confirmed
geometric violations remain differences in full-board reporting, not proven
false positives. Artifacts: `isolated-mask-pairs/summary.json` and its board,
project and report files. An initial pcbnew-removal script hit a SWIG GetTracks
binding failure; switched to source-span S-expression filtering, preserving
the retained geometry exactly.

The two U1 false positives come from the octagonal clearance approximation
around corners, one involving a track and the other a circular via. A corner
test at 45 degrees did not reproduce it; the off-axis case with corner offset
(0.24,0.10) mm did fail, reporting a mask violation despite 0.21 mm actual
copper clearance against 0.20 mm expansion. KiCad independently confirms this
simple track case is clear; the reference is in `solder-mask-corner/`.

Added a Euclidean narrow-phase distance refinement for tracks and circular
vias against pad geometry, including circle and rounded-rectangle pad data
when present. Broad-phase search remains unchanged. All three mask tests pass,
including the additional via-corner case. Rechecking the finished LPC route
removes exactly U1.1 and U1.39: **113 -> 111 internal reports**, consisting of
107 matching full-board KiCad pad/net/layer keys and the four independently
confirmed isolated pairs. This still does not claim identical grouping or
complete mask coverage. Updated artifact: `lpc-nominal-exact-mask.json`.

### Independent checker landing criterion and Encoder control

The user explicitly requested that checker accuracy improvements land independently
of the routing experiments when they agree better with KiCad. Keep the checker
on `fix/kicad-mask-drc`; do not make its acceptance depend on nominal clearance.
External KiCad remains the benchmark referee.

Audited the completed nominal EncoderBoard session without rerouting. Internal
mask reports: 0; KiCad routing-related mask reports: 0 (39 total mask reports,
including original-board geometry). Matched 137/150 original pads; eight router
pins named Via0703_GND_2 through Via0703_GND_9 are synthetic and unmatched.
This is a useful negative control, with incomplete metadata coverage explicitly
retained as a limitation. Artifacts: `/tmp/quality-kicad-mask-pilot/encoder-audit/`.

Review of KiCad 10.0.3 solder-mask provider also confirms explicit footprint
allow-bridge and net-tie exemptions. The current checker does not yet import
these exemptions; address them before calling the checker ready to land.

### Full mask-floor run completed: quality-kicad-mask-01

All 751 boards finished and all 751 KiCad referees report `ok`. Export and detailed
referee results copied to `/tmp/quality-kicad-mask-pilot/`; comparison saved as
`quality-kicad-mask-comparison.json`. Nonzero violations confirmed; no referee
repair needed. The next frozen dogleg/failure-first candidate started automatically.

| Group | U improved/regressed vs main | Net U | Routing DRC gainers | Net routing DRC | Mask gainers | Net mask reports | CPU ratio |
|---|---:|---:|---:|---:|---:|---:|---:|
| PCBench, 740 KiCad | 124/53 | -131 | 28 | -21 | 7 | -3810 | 0.8451 |
| Local, 11 KiCad | 3/0 | -17 | 1 | -2 | 0 | 0 | 0.9681 |

Absolute totals: 5416 unrouted, 983 routing violations, 10024 mask reports,
14145 all-error reports, 32321.64 CPU seconds. Fully connected boards: 574,
against baseline 526 and nominal-only 575. These are KiCad *reported* counts;
do not infer complete geometry counts from reports that may hit KiCad limits.

Versus nominal+smoothing alone, PCBench loses 332 connections (12 boards improve,
22 regress), adds 12 routing violations (6 gainers), and removes 4050 mask reports
(78 improve, 1 regresses). CPU ratio 1.0145. Local results unchanged, CPU ratio
0.9955. This is not automatically accepted: three boards account for 313 added
unrouted versus nominal (Librecalc autosave +116, Own-Mailbox mailbox +99 and
eth +98). Inspect original designer routing and candidate failure state for each.

### Intentional mask bridge permission

The browser importer already rejects net-tie footprints, so that unsupported
case does not silently reach native DRC through the browser. Footprint
`allow_soldermask_bridges` did reach import but lost its permission. Added a
KiCad 10.0.3 oracle derivative: same pad/foreign-track geometry, explicit
footprint permission, no KiCad mask error. Browser test failed with missing
metadata; native DRC test failed with 1 report when permission=true (expected 0).
Preserved permission per pad through browser DTO, native import, copy, equality,
and checker. All 4 targeted mask tests and 47 web tests pass. The permission
must not exempt ordinary copper clearance; added that assertion before starting
full workspace validation (`quality-kicad-mask-drc-workspace-02.log`).

### Checker validation complete; independent corpus candidate queued

Full workspace suite completed successfully: 2570 passed, 0 failed, 77 ignored.
Log: `/tmp/quality-kicad-mask-drc-workspace-02.log`. Web: 47 passed. This includes
the allowed-bridge test retaining ordinary copper-clearance diagnostics.

Frozen source transferred to workbench helper `~/copperroute-kicad-drc`, based
on e9d10c2. All 31 transferred file hashes verified. Source archive and manifest:
`/tmp/quality-kicad-drc-source.tar.gz`, `quality-kicad-drc-source-files.sha256`.
An incidental KiCad .prl is present in the frozen transfer but removed from the
local proposed change; it does not affect compilation. Do not mutate queued source.

Queued driver PID 336231 waits for retry driver 285526, then builds the checker
and routes the 751 KiCad boards with normal referee environment. Run ID
`quality-kicad-drc-01`, candidate `mask-drc`; log `/tmp/quality-kicad-drc-full.log`.
This DSN routing run validates unchanged routing quality and measures checker
cost on ordinary inputs. DSN lacks pad-mask metadata, so this alone cannot prove
new mask-check accuracy; finished-session metadata audits and native KiCad
oracle tests provide separate evidence for that behavior.

### Root-cause experiment: impossible pad mask clearances

The three largest regressions share the iMX233 LQFP128 footprint, including
identical IC1 placement and nearby power circuitry. IC1 pin pitch is 398.78 um,
width 203.2 um, leaving 195.58 um copper gap. Original board mask expansion is
254 um. Thus a neighboring mask aperture already reaches into the fixed pad
copper; enforcing that margin as new routing clearance can block the escape.
Original KiCad reports include track/pad mask errors at IC1; all three designer
boards were fully connected with zero routing-category DRC violations.

Inspected original mailbox tracks at IC1 pads 97-99: all 0.2 mm wide, alternating
inward/outward (97 right 0.61986 mm, 98 left 0.72892 mm, 99 right 1.22230 mm).
Geometry extracted in `regression-audit/original-mailbox-escape-tracks.json`.
Mailbox and eth mask candidates complete all 10 passes with 101 unrouted; only
Librecalc self-reports TIMED_OUT. This is not simply three timeouts.

Local diagnostic `mailbox-mask-pilot-01` uses the unchanged, hash-verified pilot07
binary (eabf5850...), dogleg enabled, failure-first disabled. Control repeats
full masks; experiment caps only IC1's mask floor to measured 195.58 um, changing
metadata only. This named-component intervention is diagnostic and not a proposed
production policy. Control result: 101 unrouted, 0 routing DRC, 200 mask reports,
80.90 CPU s, 89 vias, 1189.6145 mm. It reproduces the full-corpus failure.
Cap candidate is still running; no effectiveness claim yet.

### Mailbox cap diagnostic completed; KiCad count limit confirmed

Capping IC1 metadata to the existing 195.58 um pad gap recovers 98 connections:
101 -> 3 unrouted; routing DRC 0 -> 0; reported mask 200 -> 200; CPU
80.90 -> 110.41 s (1.3648x), vias 89 -> 181, length 1189.6145 -> 2306.9285 mm.
The latter is 0.9726x designer length and 0.8578x designer vias. Both referees
are valid and both routers completed 10 passes. Control source binary is
identical for both candidates; only copied sidecar metadata changes.

However, equal mask counts do NOT prove unchanged mask quality. Inspected
KiCad 10.0.3 `pcbnew/drc/drc_engine.cpp`: ERROR_LIMIT=199; RunTests uses this
for mask errors (499 only for clearance and unconnected items). Parallel
providers can slightly exceed the threshold. The mask provider stops work or
emission when its error limit is exceeded. CLI --all-track-errors does not
raise this per-type cap. Primary source:
https://github.com/KiCad/kicad-source-mirror/blob/10.0.3/pcbnew/drc/drc_engine.cpp

This confirms the escape-blocking hypothesis but leaves mask acceptance open.
Next: assess mask contacts with local KiCad probes below the report limit,
and test a geometry-derived cap rather than a named-component override.
Keep complete-board standard KiCad results as the benchmark, explicitly
acknowledging saturated violation counts rather than asserting full parity.

### General pad-gap cap prototype and unsaturated KiCad probes

Replaced the named-component diagnostic with a geometry-derived candidate:
limit each requested mask clearance by the gap to existing foreign-net pad
copper on that layer. Same-net pads and movable routing are not limiting
obstacles; the subsequent clearance-class update still takes the maximum with
all preexisting forward/reverse copper rules. This only relaxes the proposed
mask floor, not existing copper-clearance guards. Native mask preparation uses
the helper; DSN-sidecar experiments enable it with temporary
`COPPERROUTE_MASK_GAP_CAP` for a controlled same-binary comparison.

TDD fixture: two 100-unit-wide pads with a 300-unit gap, requested mask floor
400. Initial test assertion for the existing asymmetric wide class used the
wrong direction; corrected it. Then observed the intended failure: foreign-net
neighbor gave 400, expected 300. Implementation passes all four pad-clearance
module tests, including same-net (400), other layer (200), and existing stricter
reverse copper rule (600). Logs: `/tmp/quality-kicad-mask-cap-{red,green}.log`.

A separate KiCad diagnostic retains all copper and exposes one IC1 pad mask
aperture per copy, using --all-track-errors. All 128 apertures are checked for
original, main, full-mask and capped-mailbox routes (512 probes). This removes
full-board aperture grouping, so sums are contact diagnostics, not a replacement
benchmark. Every probe asserts fewer than 199 mask reports. Original/main
completed so far: 246/242 routing-involved reports. Full and cap are ongoing.
Driver `/tmp/quality-kicad-mask-probes.mjs`; log same prefix .log; reports and
partial results in `regression-audit/mailbox-mask-probes/`.

Previous pilot07 release binary preserved at `/tmp/quality-kicad-mask-pilot/pilot07-router`
before rebuilding the general cap candidate. Prepared local `mask-gap-pilot-02`
on the three regression boards, with full-mask control versus geometry cap,
same binary, failure-first disabled; launch only after release build completes.

### General gap cap pilot completed on all three major regressions

`mask-gap-pilot-02` completed with six valid KiCad referees. Same binary
(a2cdda6ae55f8c2cb1dc36fb63afd3adba7a29f392dfb4c1a383afca3da28778),
full-mask control versus geometry-derived cap, failure-first disabled:

| Board | Unrouted control -> cap | Routing DRC | Reported mask control -> cap | CPU s control -> cap |
|---|---:|---:|---:|---:|
| Own-Mailbox mailbox | 101 -> 0 | 0 -> 0 | 201 -> 202 | 82.73 -> 90.15 |
| Own-Mailbox eth | 101 -> 0 | 0 -> 0 | 199 -> 201 | 89.17 -> 76.07 |
| Librecalc autosave | 117 -> 3 | 0 -> 0 | 199 -> 203 | 115.83 -> 116.63 |

Net -316 unrouted, three boards improve and none regress; no routing DRC
gainers; +7 reported mask errors, all three counts saturated. CPU ratio
282.85/287.73 = 0.9830, not a timing claim. Librecalc control self-reports
TIMED_OUT; cap completes. KiCad probe work ran concurrently on this laptop,
so this is a local quality pilot, not a performance comparison suitable for PR.
Cap wirelength is 98.89%, 96.01%, 98.57% of original designer routing;
vias are 87.20%, 80.19%, 74.43%, respectively. Complete artifact:
`/tmp/quality-kicad-mask-pilot/gap02-results.json`.

The earlier named-IC1 cap probe audit completed all 512 cases: original/main/
full-mask/named-cap routing mask contacts at IC1 = 246/242/0/216; total probe
mask reports = 535/486/244/460. Versus main, 37 pad probes improve and 19 regress.
These contact results apply to the named cap diagnostic, not automatically to
the later geometry-derived candidate. No general-cap corpus acceptance yet.

The user requested the solder-mask checker PR be wrapped separately and soon.
Prioritize its completed accuracy audit and pending corpus run; defer further
routing-cap experiments until the checker PR is ready. The routing sources and
results remain preserved for resuming that work.

### Full dogleg/retry run completed: quality-kicad-retry-01

751 boards finished. One PGA2311 KiCad SES import failed with GTK diagnostics;
rescoring on a fresh display succeeded, with the SES hash unchanged. All 751
referees now report `ok`. Re-exported and copied results/details locally.
The independent checker run started automatically when this driver exited.

| Group | U improved/regressed vs main | Net U | Routing DRC gainers | Net routing DRC | Mask gainers | Net mask reports | CPU ratio |
|---|---:|---:|---:|---:|---:|---:|---:|
| PCBench, 740 KiCad | 126/55 | -115 | 30 | -30 | 10 | -3809 | 0.8571 |
| Local, 11 KiCad | 3/0 | -18 | 1 | -2 | 0 | 0 | 1.0177 |

Absolute totals: 5431 unrouted, 974 routing violations, 10025 mask reports,
14137 all-error reports, 32806.67 CPU seconds, 570 fully connected boards.
Against the preceding mask-floor run: PCBench 47 improve/39 regress in unrouted,
net +16 U, -9 routing DRC (20 gainers), +1 mask report (7 gainers), CPU ratio
1.0142. Local 1 improves/1 regresses, net -1 U, unchanged DRC, CPU ratio 1.0512.
Combined net +15 U, -9 routing violations versus the mask-floor candidate.

Do not reject automatically from that +15: Blitz_Rev.K_C68 alone contributes
+90 reported U (409 -> 499). All compared Blitz runs time out during pass 1;
internal incomplete counts are main 712, mask 401, retry 538. KiCad's 499
unconnected-item report limit caps main/retry figures. Failure-first ordering
has no prior-pass failures to prioritize here, so this cannot yet be attributed
to that scheduling change rather than the bundled dogleg or class-preservation
change. Investigate the ablations after the checker PR. PCBench excluding this
one board improves by 74 reported U versus the preceding mask run.

One low-priority two-job checker prebuild overlapped the latter portion of this
run; no timing improvement is claimed from these single-run CPU observations.
Artifacts: `quality-kicad-retry-{details,comparison}.json` and the exported JSON
under `/tmp/quality-kicad-mask-pilot/`. No new routing change accepted yet.

### Mask regressions outside pad apertures: artwork

Examined original KiCad boards and exported external DRC reports for two
nominal/smoothing regressions that the pad-floor prototype does not improve.
`8bit-cpu_programming_interface` reports 0 mask violations on main, 32 on
nominal/smoothing, and 32 with pad floors. All 32 prototype reports involve
segments on F.Mask. The original contains these mask graphics (for example
UUID 00000000-0000-0000-0000-00005e80d990); its imported raw DRC reports zero
mask errors. `FRM16_Relay_Module_I2C_Controller_relay_controller` reports
5 / 41 / 41 respectively. All 41 prototype reports involve B.Mask artwork:
31 AndrewSowa.com text contacts, nine polygon contacts, and one Made In
Chicago text contact. The original explicitly places the text on B.Mask;
its raw DRC likewise reports zero mask errors. Raw reports are historical
import evidence, not a fresh matched-version original-board comparison.

This identifies a missing input category: raising pad clearance cannot
protect independent mask graphics. Next experiment should preserve those
openings and model their interaction with routed copper, validated by
fresh KiCad original and candidate checks. Do not silently treat all mask
artwork as forbidden copper: KiCad's bridge semantics need a minimal oracle
first. No code change yet. Artifacts on laptop:
`/tmp/quality-kicad-mask-pilot/new-mask-regressions/`.

A minimal KiCad 10.0.3 artwork oracle now confirms the distinction: one
F.Mask line crossing no tracks, one net, or two tracks on the same net
produces zero mask reports; crossing two distinct nets produces one.
Pads anchor track nets, and the generator reloads the saved boards and
asserts their track-net assignments to prevent an invalid oracle caused
by automatic net reassignment. Files and reports:
`/tmp/quality-kicad-mask-artwork-oracle/`; generator beside that directory.
Therefore a blanket copper keepout for every mask graphic would reject
legal single-net use. A faithful structural solution needs aperture-level
net occupancy, including updates during rip-up, rather than pad floors.
This remains a separate routing follow-up, not part of the frozen checker PR.

Fresh KiCad 10.0.3 checks of both original boards, with their original
projects and `--all-track-errors`, confirm zero solder-mask reports and
zero unconnected items. Relay controller has zero total error-severity
violations; programming interface has 36 other errors. Thus their mask
artwork can coexist with complete designer routing. Results are saved in
`new-mask-regressions/original-fresh-summary.json` and per-board
`raw-drc-fresh.json` beneath the laptop artifact root above.

Programming-interface geometry follow-up: a direct KiCad shape-overlap
audit of the 223 F.Mask line objects initially found no multi-net
contacts when considering only tracks, pads and copper graphics. Including
filled zones identifies 30 apertures touching multiple nets in the pad-floor
result, versus zero in the original. This reinforces that mask occupancy
must include copper fills; a track-only ownership model is incomplete.
The zero-clearance overlap diagnostic is not identical to KiCad DRC (32
external reports) and is not a replacement score. Script:
`/tmp/quality-kicad-artwork-occupancy.py`; full contact data in
`new-mask-regressions/8bit-cpu_programming_interface/artwork-occupancy.json`.

### Solder-mask checker PR ready; full geometry-gap candidate launched

Separate checker PR #22 is ready for review at commit 36283c2, with the
failed standard gate and all diagnostic limits retained. Four longer
controls converge exactly; decelerator still times out but has equal
quality counts. All five 100-item controls have identical SES files.
No checker code is being merged into main automatically.

Started `quality-kicad-gap-01`, candidate `nominal-mask-gap`, in isolated
workbench checkout `~/copperroute-kicad-gap` at base e9d10c2. Thirty source
files match the hashes from the local gap02 pilot exactly. The candidate
contains nominal clearance, smoothing, dogleg escape, no-op class guard
and the geometry-derived mask-floor cap. Failed-first scheduling is
explicitly disabled, as in the successful local gap02 comparison.
The frozen pilot also includes its older native metadata/checker
preparation; the benchmark remains DSN plus sidecar, externally scored
by KiCad. Do not replace these sources with PR #22 mid-run.

Pad metadata is copied into the candidate checkout and hashed; neither
the common source directory nor a running candidate will be edited.
Build then run all 751 KiCad boards, ten passes, 300 seconds, one thread,
twelve jobs. Driver `/tmp/quality-kicad-gap-full.sh`, PID 466990; log
`/tmp/quality-kicad-gap-full.log`. Previous checker/control jobs were
verified finished before launch. Compare against main and the nominal,
mask-floor and retry experiments; do not infer the full effect from the
three-board local recovery of 316 connections.

KiCad-only scoring tooling is now ready for review separately:
https://github.com/emshotton/copperroute/pull/23 (existing commit 7243118,
new branch bench/kicad-only). Fresh benchmark suite: 257 passed, one
optional integration skip. No Rust source changes; CONTRIBUTING's tooling
exception applies. The frozen workbench runner is not changed mid-run.

Early mask-gap referee sanity check: 22 scores, all valid, including 14
PCBench boards; five ordinary violations and 133 mask reports confirm
that KiCad is actually scoring. Too early for a quality conclusion.

Structural investigation started: the current maze rip-up resolver prices
obstacles using pass cost, width, detour, fanout protection and seeded
randomization; the pass runner initializes its ripped-item cost map for
each connection. Before proposing history-based costs, measure actual
repeated disruption and distinguish it from intrinsically slow maze
searches on timeout boards. No algorithm patch or result claim yet.

Structural diagnostic now runs in isolated branch experiment/congestion-history
at `../copperroute-congestion-study` (base e9d10c2). Temporary env-gated
logging records connection start/end, elapsed attempt time, state, and the
nets actually removed before insertion. No cost, ordering, clearance or
permitted-move behavior is changed. The instrumented release binary was
built with RUSTC_WRAPPER disabled; its hash and source patch are saved in
`/tmp/quality-congestion-study/`. Diagnostic inputs: boatcontrol CommonCathode60A,
uSKY and oasis ledboard, chosen because they complete quickly with remaining
connections in the full main baseline. Original designer boards and raw DRC
are copied alongside inputs. Ten passes, one thread, 120-second local cap;
these timing-limited diagnostics are not quality comparisons.

Relevant primary implementation reference: VTR documents separate present
and historical congestion factors, growth limits and search bounds:
https://docs.verilogtorouting.org/en/latest/vpr/command_line_usage/ .
This motivates testing nonuniform congestion history, but FPGA routing
resources differ from CopperRoute's gridless PCB geometry. Applicability
remains a hypothesis until the diagnostic shows recurrent disruption.
The original PathFinder paper was not retrieved successfully; do not imply
that its full algorithm has been reviewed or reproduced.

First structural diagnostic results (all logging-on/off pairs produce
identical finished SES files): boatcontrol has 1,234 main-router failures
and only two rip-up net events; oasis has 116 main-router failures and no
rip-ups. uSKY has 437 main-router failures, 36 successful route attempts
and 34 attributable main-router rip-up net events. Its 704 raw rip-up net
events must not be called main-router oscillation: 40 occur within
optimizer pass-runner attempts and 630 are outside the instrumented
pass-runner scopes. Further scope instrumentation is needed for those.
The common 10↔11 pair occurs four times in one direction in main routing;
the larger reciprocal counts come from optimizer/other activity.

All three original designer references are fully connected with zero
ordinary routing violations. Ground-truth wirelength/vias: boatcontrol
3,624.34mm/0; uSKY 174.59mm/23; oasis 93.99mm/0. The next step is geometry
and failure-reason inspection, not assuming that a history cost solves
all three. Diagnostic summaries, raw logs, references, and observer
controls are saved in `/tmp/quality-congestion-study/`.

uSKY designer comparison: 23 original vias; 21 use diameter/drill
0.6858/0.3302mm, exactly the DSN's available Via[0-1] definition; the
other two are larger (0.889/0.508mm). Every measured Rust variation so
far uses zero vias on this board (main/nominal/mask/retry unrouted
26/26/26/25). Missing a smaller via size therefore does not explain the
zero-via result. The next diagnostic adds effective layer/via settings
and failure details, to distinguish disabled layer transitions, inability
to find a route, and inability to insert a found route. No fix yet.

Root cause found for uSKY's missing main-router vias: library import
normalizes `Via[0-1]_685.8:330.2_um` to `Via[0-1]_685:330_um`, while
create_via_rule compares the unnormalized use_via reference. The via's
clearance class is 1 on both sides, so the earlier clearance-mismatch
hypothesis was rejected (its minimal test passed before any fix).
Fanout's board-via fallback explains why some diagnostic calls can still
see a via while main routing cannot.

TDD: minimal fractional-name test fails 0 usable vias versus expected 1.
The proposed import fix keeps exact matching and adds normalized matching,
retaining the clearance-class filter and SMD-attachment restriction.
Both named/default class cases are covered. Twelve relevant network,
JVM-scope and KiCad-permission tests pass. No JVM expectations rerecorded.
This is a correctness prerequisite found during structural diagnosis, not
a substitute for the planned algorithm experiments. Local KiCad pilot
prepared for uSKY plus boatcontrol and oasis controls, baseline/candidate
binaries frozen separately with temporary diagnostic logging disabled.

Via-name local KiCad pilot complete: three PCBench boards, both candidates
finish, all six referees valid. uSKY unrouted 26 → 24, vias 0 → 4;
boatcontrol remains 95 unrouted and oasis remains 11. One board improved,
zero regressed, net −2 unrouted. Ordinary violations stay zero on all
three; no violation gainers; mask reports also unchanged at zero. Total
all-category errors stay 39. CPU 20.62 → 23.18s, ratio 1.1242, one local
measurement only. This fixes an actual unavailable-via rule but does not
by itself route uSKY completely.

Clean fix worktree `../copperroute-via-name`, branch fix/dsn-via-name,
contains only the importer change and regression test. No diagnostic
logging. Full workspace tests are running locally. Queued isolated
workbench run `quality-kicad-via-name-01`, candidate via-name, based on
e9d10c2 plus those two hashed files. PID 511506 waits for mask-gap
rescore PID 508912; build and all 751 KiCad boards follow (ten passes,
300 seconds, one thread, twelve jobs). Frozen source checksum file:
`/tmp/quality-kicad-via-name-source-files.sha256`. No fix commit or PR
until corpus results have been assessed.

Mask-gap run has GTK session-import failures; rescore PID 508912 waits
for routing PID 466990, preserves initial results, uses a fresh display
per failed cell, verifies unchanged SES hashes, and asserts no failed
referees remain before exporting again. Do not compare unscored rows.

### New EPYC experiment host (2026-09-09)

User reauthorized the temporary host root@185.189.44.159. Verified AMD EPYC 9654, 96 physical cores / 192 hardware threads, 755 GiB RAM, Ubuntu 24.04. Installed baseline host tools and began a direct workbench-to-server copy of the frozen 751-board KiCad corpus and exact KiCad 10.0.4 / Python Nix dependencies. Existing workbench experiments remain untouched. The runner source is the KiCad-only tooling branch 7243118; the baseline router is the exact workbench e9d10c2 binary. Bootstrap will verify tool imports and score EncoderBoard and LPC2148 before any full run is trusted. No new-server routing results yet.

A laptop-owned backup loop copies results, exports, and reports every minute to `/Users/em/Development/freerouting/copperroute-epyc-results/`, without deleting prior copies; script `/tmp/quality-epyc-backup.sh`, log `/tmp/quality-epyc-backup.log`. Bootstrap script `/root/quality-epyc-bootstrap.sh`, log `benchmark/reports/epyc-bootstrap.log`. Start around 96 concurrent single-threaded boards and measure whether SMT helps before using 192. Any speed/quality comparison needs a same-host baseline because timeout outcomes and CPU timing can differ across CPUs.

EPYC setup complete: exact KiCad 10.0.4 CLI and pcbnew both verified; EncoderBoard matched workbench (1 U, 0 ordinary DRC, 39 mask). LPC2148 completed in 179.62 seconds (1 U, 8 ordinary DRC, 212 capped mask); workbench's same baseline timed out at 300 seconds (8 U, 16 ordinary DRC, 199 capped mask). This is hardware/deadline behavior, not a routing improvement. The full same-host main versus via-name comparison `quality-epyc-via-01` is running with **192 jobs**, per the user's explicit hyperthreading preference. First utilization sample: 98.71% aggregate CPU, 189.52 busy logical CPUs, ~16 GiB system memory used. Main uses the exact workbench baseline binary; via-name uses the same Rust 1.97.1 toolchain and frozen source, compiled on Ubuntu. Runtime/system-linker differences mean workbench's pending same-build-environment comparison remains useful for timing confirmation.

Full results are now backed up to workbench (`~/copperroute-epyc-results`) every minute; laptop backup is limited to exports/reports because only 14 GiB is free locally. Live-copy rsync code 24 from transient KiCad lock files is expected; completed cells are recopied and must be checked after completion. Driver `/root/quality-epyc-full-via.sh`, PID 16791; utilization monitor `/root/quality-epyc-monitor.py`. The two-board smoke run is complete; the 1502-cell comparison is not complete.

Via-name test review: three JVM parity tests failed specifically because their recordings retain an empty fractional-name via rule. Isolated review worktree `copperroute-via-name-review` preserves the recordings and explicitly asserts the sole intentional correction for each fixture; all 19 rules tests now pass. Full workspace suite running, no commit yet. The frozen corpus candidate source is unchanged.

### Failed-insertion restoration pilot and bus-routing lead

On uSKY after fixing fractional via names, diagnostic connection-component counting observed 13 failed main-pass attempts involving rip-up; 12 left more disconnected pin groups, with a total 18 additional groups across those intermediate events. Not a final unrouted delta. Instrumentation preserved byte-identical SES (SHA256 472b8ff1ad03a53ecd843457d21eb84fd6e033bf64c0d8f7bd964060ae6fc9d3). Data: `/tmp/quality-congestion-study/uSKY_uSKY/insert-diagnostic-events.json`.

New isolated `experiment/insert-rollback` prototype, based on e9 plus the via-name fix: snapshot before destructive rip-up, restore on normal insertion failure, only for non-retained autoroute databases. Error/panic handling and retained databases are not covered by this prototype. A small crossing-track fixture with an insertion width exceeding its search envelope fails on the original code because its original track disappears; after restoration all 18 autoroute-connection tests pass. Initial test's equality also compared pre-initialization tree caches; corrected to snapshot after engine initialization, reran red (missing track) then green.

Six-cell local KiCad pilot: uSKY 24→22 unrouted, boatcontrol 95→95, oasis 11→11. All ordinary DRC and mask counts remain zero, all six complete. CPU 23.56→27.51 s, ratio 1.168. This is a pilot, not a corpus-accepted change. Full via-name review suite separately passed 2566 tests, 77 ignored.

User suggested learning from grouped, parallel middle runs with fanout at endpoints. Research notes and a proposed corridor/escape experiment are in `docs/bundle-routing-research.md`; not implemented yet. This remains part of the requested larger structural algorithm work.

### EPYC via-name comparison complete

`quality-epyc-via-01`: 751 boards per candidate, all 1502 referees now valid. Thirty-nine initial failures (SES-import failures and font-warning prefixes before structured statistics) were rescored without rerouting; route SHA256 checked unchanged, initial reports retained. Warning-prefix repair preserves the raw output and log, accepts only one JSON object on the final output line; it does not alter KiCad DRC decisions. Artifacts on workbench, laptop reports/exports backup, plus `/tmp/quality-epyc-bootstrap/{main,via}-details.json`.

pcbench, 740 boards: U 5260→4514, improved/regressed 53/1; ordinary DRC 940→917, 1 boards gaining violations; mask 15183→15221, 26 mask gainers; CPU ratio 0.9599.

kicad, 11 boards: U 240→239, improved/regressed 1/0; ordinary DRC 59→59, 0 boards gaining violations; mask 0→0, 0 mask gainers; CPU ratio 1.0209.

Need inspect regression boards, mask changes, same-SES comparisons and timeout effects before acceptance. No merge claim yet. A fresh same-server main build versus the frozen nominal/smoothing/dogleg/mask-gap candidate is now building/queued in `quality-epyc-gap-01`, 192 jobs; this also makes both sides use the same linker environment. Workbench gap and via-name validation remain in progress/queued.

Rollback prototype full workspace suite completed successfully (log `/tmp/quality-insert-rollback-workspace.log`); no commit yet. Frozen five-file source shipped to EPYC and hash-verified. `quality-epyc-rollback-01` will compare via-name alone with via-name plus failed-insertion restoration at 192 jobs, after the gap run and rescoring. Driver PID1892840, rescore waiter PID2209802. Gap rescore waiter PID1728197; gap utilization monitor PID2209803. All reports/exports and full results retain periodic laptop/workbench backups.

Via-name's sole connection regression is Apple M0110: main 0 U/0 vias/4356.05 mm versus fix 1 U/2 vias/4320.13 mm, both completed. KiCad identifies a missing Col9 connection between back-layer tracks near (164.680,82.375) and (197.940,52.095) mm. Its original human board is fully connected with 14 vias and 4509.26 mm. A local main/via-name/rollback comparison is now running to test whether failed-insertion recovery addresses this particular regression. Do not reject the broad improvement solely because of this one board.

Of the 751 via-name comparisons, 638 SES files are byte-identical. The largest apparent mask increase (BLDC controller +33) is on identical routing. Changed-route mask gainers still need investigation; e.g. kitspace esp8266 +18, bobc MS-F100 +25, teensy-touch +10. The overall ordinary DRC increase on nonSNES is +2, accompanied by 53 fewer unrouted connections, with both candidates timed out. These details qualify the encouraging aggregate result.

Bus-routing original-board comparison is now concrete: NRC2016 shows the human's coordinated horizontal trunks and connector fanout versus CopperRoute's extra lower-edge detours. Selected-net length 1890.8→2210.9 mm, vias 20→22. See the reproducible side-by-side figures and notes in `docs/bundle-routing-research.md`. No grouped-routing implementation yet.

Apple M0110 targeted local KiCad pilot completed: main 0 U / 0 ordinary DRC / 78 mask / 0 vias / 3.54 CPU s; via-name 1 U / 0 DRC / 72 mask / 2 vias / 12.11 s; via-name plus failed-insertion recovery **0 U / 0 DRC / 75 mask / 2 vias / 2.10 s**. All completed. The recovery prototype fixes the only unrouted regression seen in the EPYC via-name run in this local test. Mask counts are below the original main here but +3 versus via-name; corpus acceptance remains pending. Data `/tmp/quality-congestion-study/apple-via-rollback-pilot-results.json`.

### Workbench geometry-gap cap full run complete

`quality-kicad-gap-01`, nominal-mask-gap: all 751 KiCad referees valid after repairing 12 failed imports without changing routes. Details saved locally as `/tmp/quality-kicad-mask-pilot/quality-kicad-gap-details.json`. Main comparison below separates PCBench and local KiCad fixtures; no Java DRC population is included.

pcbench- (740): unrouted 5321→5082, 117 improved / 57 regressed; ordinary DRC 947→903, 28 gainers; mask 13834→10135, 11 gainers; CPU ratio 0.9100.

kicad- (11): unrouted 243→226, 3 improved / 0 regressed; ordinary DRC 59→57, 1 gainers; mask 0→0, 0 gainers; CPU ratio 1.0259.

Totals versus main: **256 fewer unrouted, 46 fewer ordinary DRC, 3699 fewer reported mask violations**. Versus nominal+smoothing, however, there are 224 more unrouted (PCBench), 11 fewer ordinary DRC and 3939 fewer mask reports, with PCBench CPU ratio 1.0924. Thus this fixes much of the solder-mask tradeoff but does not yet retain all of nominal routing's connection gain.

Versus the original mask-floor run, PCBench improves by 108 unrouted / 23 ordinary DRC but adds 111 mask reports and CPU ratio 1.0768. The three related boards recover 314 connections: mailbox 101→1, eth 101→0, Librecalc autosave 117→4. Timeout regressions offset much of this: Blitz RevK +90, Blitz copy +45, ReSDMAC +33, all timed out. Do not attribute all of that to the geometry cap: this bundle also includes dogleg and no-op clearance-class guard.

Largest completed-board regression against main remains avr-fuser-32 adapter: 0→26 unrouted and ordinary DRC 3→25, while mask reports 202→0. Types change from 2 shorts/1 clearance to 13 shorts/12 clearances. It was already bad under the original mask floor (26 unrouted, 25 ordinary DRC). This needs root-cause analysis rather than dismissing the board or simply counting its mask reduction as success.

EPYC gap routing is also now complete; its 35 failed referees are being repaired before comparison. Workbench via-name run has been released from its wait and is building/running. EPYC rollback run remains queued behind gap scoring.

EPYC gap comparison fully scored (751/751 per side), independent same-host main build using same compiler/linker. Detailed comparison saved `/tmp/quality-epyc-bootstrap/gap-comparison.json`; rows:

```json
[
  {
    "group": "pcbench-",
    "boards": 740,
    "unrouted": [
      5249,
      4690
    ],
    "improved": 124,
    "regressed": 50,
    "violations": [
      940,
      906
    ],
    "violation_gainers": 28,
    "mask": [
      15245,
      10958
    ],
    "mask_gainers": 9,
    "cpu_ratio": 0.8583060905683659
  },
  {
    "group": "kicad-",
    "boards": 11,
    "unrouted": [
      241,
      226
    ],
    "improved": 3,
    "regressed": 0,
    "violations": [
      59,
      57
    ],
    "violation_gainers": 1,
    "mask": [
      0,
      0
    ],
    "mask_gainers": 0,
    "cpu_ratio": 0.9178945635429283
  }
]
```

Rollback prototype build has started after gap scoring finished.

### Failed-insertion restoration: full EPYC comparison

`quality-epyc-rollback-01` is complete, all 751/751 KiCad referees valid for via-name control and via-name plus restoration. Exact source frozen in `/root/copperroute-insert-rollback`; five-file hashes `/tmp/quality-insert-rollback-source.sha256`. Full workspace passed 2567 tests, 77 ignored before this result; no commit yet.

```json
[
  {
    "group": "pcbench-",
    "boards": 740,
    "unrouted": [
      4713,
      4449
    ],
    "improved": 64,
    "regressed": 49,
    "violations": [
      915,
      833
    ],
    "violation_gainers": 13,
    "mask": [
      15184,
      15196
    ],
    "mask_gainers": 34,
    "cpu_ratio": 0.9780645484134413
  },
  {
    "group": "kicad-",
    "boards": 11,
    "unrouted": [
      241,
      240
    ],
    "improved": 1,
    "regressed": 3,
    "violations": [
      59,
      56
    ],
    "violation_gainers": 0,
    "mask": [
      0,
      0
    ],
    "mask_gainers": 0,
    "cpu_ratio": 1.1337901529131065
  }
]
```

Totals: 265 fewer unrouted and 85 fewer ordinary DRC; reported mask +12. There are 65 boards with fewer unrouted and 52 with more. Among 723 pairs where both finish, unrouted improves by 52; the other 28 pairs contribute -213 and must be interpreted with the timeout caveat. CPU ratio PCBench 0.9781, local fixtures 1.1338; no speed claim from a single run. Median per-board RSS unchanged at 13.6 MiB; maximum 447.3→246.4 MiB. Do not add this delta to the earlier via-name delta: the control runs differ, especially timeout outcomes.

Largest completed regressions: 96boards-sensors 1→8 unrouted, amalthea 13→19, freeDSP balanced 3→7. Largest gains are mostly timeout-limited boards (nonSNES -61, Karabas revG -34, revC -32); completed examples include BLDC controller 19→11 and boatcontrol NonLatchingNO30A 26→18. The experiment is promising but the lost partial-routing progress on regressed boards needs investigation before acceptance.

### AVR Fuser: model omissions and a combined pilot

The frozen mask-gap source was combined with the independently tested via-name correction in isolated `experiment/mask-via`, without rollback. Thirty-three source files were hash-verified and copied to `/root/copperroute-mask-via`. `quality-epyc-mask-via-01` is running against a same-host main binary at 192 jobs; driver PID3840299, rescore waiter PID609970. All prior frozen experiments remain unchanged.

AVR Fuser local 120-second pilot: gap 26 U / 25 ordinary DRC / 0 mask / 2 vias / 46.57 CPU s, COMPLETED; gap+via-name 0 U / 28 ordinary DRC / 0 mask / 67 vias / 116.42 CPU s, TIMED_OUT. Restoring via access recovers the connections, but does not solve DRC. **All 25 and 28 copper violations involve copper-layer text**, not track-to-track shorts. The DSN contains none of that copper text. The original human board is fully routed, with zero ordinary DRC and 145 vias.

The completed EPYC main copper-text audit finds 25 clearance, 27 shorting and 5 hole-clearance errors involving PCB text across the corpus (57 ordinary DRC). It explains part of the baseline; do not claim it explains all ~921 historical errors. Audit `/tmp/quality-epyc-bootstrap/copper-text-violation-audit.json`. Browser import already has conservative default-font copper-text rectangles; the benchmark's KiCad-generated DSN omits this geometry. Native/benchmark model implications need separate treatment.

Diagnostic DSN augmentation used KiCad 10.0.4 `TransformTextToPolySet` with 1 µm outside approximation, retaining polygon holes, for the 13 actual board copper texts. It adds 73 trace keepouts plus 73 via keepouts (146 total), preserving original DSN other content. This is a diagnostic input change, not a committed router fix. `/tmp/quality-avr-fuser/with-text.dsn`, generator `/tmp/quality-avr-text-dsn.py`, source shape counts `/tmp/quality-avr-fuser/text-shapes.json`.

At a matched 300-second cap with gap+via-name: original DSN 0 U / 28 ordinary DRC / 0 mask / 67 vias / 286.87 CPU s, TIMED_OUT; DSN with copper text 26 U / **0 ordinary DRC and 0 mask** / 38 vias / 46.08 CPU s, COMPLETED. Correct geometry removes DRC but exposes a routability problem. Data `/tmp/quality-avr-fuser/avr-text-pilot-results.json`. Hypothesis that the designer used smaller vias is **refuted**: all 145 original vias are diameter 1.69926 mm, drill 0.8001 mm, exactly the DSN's declared via. Next investigations: why partial-progress rollback hurts 96boards-sensors, and how ordered multi-terminal routing/escape placement can use the available layers on these difficult original-board examples.

## EPYC mask + via-name full comparison, 2026-09-09

`quality-epyc-mask-via-01` finished; all 751 boards on both sides scored successfully by KiCad. Same-host main and frozen 33-file nominal/smoothing/mask-gap + via-name candidate, 192 jobs, one thread, ten passes, 300 seconds. No failed-insertion rollback or copper-text augmentation included. Full raw results backed up on workbench; details also pulled locally.

| Group | Unrouted main → candidate | U improved / regressed | Copper DRC main → candidate | Copper gainers | Mask main → candidate | Mask gainers | CPU ratio |
|---|---:|---:|---:|---:|---:|---:|---:|
| PCBench / KiCad (740) | 5250 → 4253 | 139 / 39 | 936 → 881 | 26 | 15322 → 10940 | 8 | 0.8721 |
| Local / KiCad (11) | 241 → 226 | 3 / 0 | 59 → 57 | 1 | 0 → 0 | 0 | 0.9198 |

Combined improvement: 1012 fewer unrouted, 57 fewer copper DRC, 4382 fewer reported mask violations. Remains experimental: 39 U regressions, 27 copper-DRC gainers. Worst U regression GB-CART256K-A 3 → 11, both completed, followed by dorkyboard 0 → 4, rxadc14 0 → 4, kinetoscope microcontroller 51 → 55, EnvOpenPico 15 → 19. These require original-board and result inspection before claiming mergeability. CPU is a single-run ratio, not a timing claim; capped mask reporting and deadline effects still apply. Raw summary `/tmp/quality-epyc-bootstrap/mask-via-comparison.json`.

### Via-name correction prepared for review

PR24 https://github.com/emshotton/copperroute/pull/24 is open, commit `6a46edb`, with the frozen measured production fix, explicit JVM divergences and full benchmark report. Fresh workspace suite exited zero (2567 passed, 77 ignored). Report preserves the failing automatic quality gate and Apple M0110/mask tradeoffs. No mask or insertion-recovery code is included.

### Isolate nominal clearance and smoothing on current regression boards

The largest completed mask+via U regression, GB-CART256K-A, is 3 → 11 with nominal+smoothing already, and has exactly the same 93 vias / 1921.3062 mm with mask-floor, mask-gap and mask-gap+via. Main and via-name-only each have 3 U, 2 copper DRC, 91 vias / 1831.92 mm. Original human board has 0 U, 0 recorded DRC, 122 vias / 1938.3092 mm. Thus this is not introduced by the mask guard or restored via-name option.

Prepared diagnostic worktree `copperroute-routing-ablation` at via-name PR24 and two environment switches selecting the previously tested nominal and smoothing behavior. No new algorithm fix. Frozen source patch `/tmp/quality-routing-ablation.patch`; source archive and switches copied to a separate EPYC helper. Run `quality-epyc-ablation-01` compares control/nominal/smoothing/both across 73 boards: the union of all U/copper/mask regressions from the combined full run, plus ADC smoothing-churn and two grouping controls. 292 routing cells, 192 jobs, 10 passes/300 seconds, same binary with process-local switches. Driver PID926861 verified live compiling. This is a selected diagnostic sample, not a full-corpus acceptance run or timing comparison. All full-run source/binaries remain untouched.

### GB-CART256K-A nominal/smoothing isolation

All four cells for this board in the selected ablation run completed and were KiCad-scored: control 3 U / 2 V / 161.32 CPU seconds; nominal 11 U / 0 V / 133.28; smoothing 3 U / 2 V / 162.35; both 11 U / 0 V / 110.29. This isolates the 8-connection regression to nominal clearance, not smoothing, on this board. Other selected-board results remain pending. CPU figures are descriptive only. The original comparison plot and extractor are preserved in `routing-quality-artifacts/bundle-study/gb-hardware_GB-CART256K-A.{json,svg}`.

Started unchanged diagnostic-binary routing of 96boards-sensors to investigate the separate recovery prototype's 1 → 8 U regression. Logging pin-component counts before/after failed insertion, with matched single-thread/10-pass/300-second settings. This is observation only, not a recovery-policy modification; do not claim connectivity-improving partial insertions unless observed.

### 96boards-sensors failed-insertion diagnostic completed

Matched diagnostic run completed with 1 internal U and byte-identical SES to the full EPYC via-name control: `173cf8a1db7b1d5a7432680a26cdd2aec8ef10cfb75436e102088814c8dd3a59`. Across 222 logged ripup/insertion events, 44 insertions reported failure: 34 increased the sum of affected pin-component counts, 10 left it unchanged, none reduced it. Thus an immediate connectivity improvement from failed partial insertions does **not** explain this board's recovery regression. Useful geometry changes or later rerouting opportunities remain possible; do not add a count-based guard as if that cause were established. Full observations `/tmp/quality-congestion-study/96boards-sensors_Sensors/insert-events.json`, matched result/SES/log alongside.

### First grouping-order prototype

New isolated worktree `copperroute-bundle-order` at via-name PR24. Hypothesis from original NRC2016 and GB cartridge layouts: globally sorting individual attempts by shortest current distance interleaves related signals, while grouping attempts may leave more consistent space for their neighbours. This tests scheduling only, not a shared-corridor planner. Net/component geometry identifies pairs sharing at least four distinct signals, ignoring plane nets and nets spanning more than six components. Largest groups claim overlapping nets first; each group orders by source endpoint projection perpendicular to the component-to-component direction. No pin reassignment, layer restriction, or clearance change. Remaining attempts retain stable order. Endpoint inversions are not solved by this prototype.

TDD ordering test failed on the unchanged queue for the expected reason, then passed. Two additional tests cover repeated-pad deduplication and input-order/rotation/translation invariance. All three pass. Full default workspace suite is running separately; benchmark enables the prototype with `COPPERROUTE_BUNDLE_ORDER=1`, leaving the control path unchanged. Frozen three-file overlay `/tmp/quality-bundle-order-source.tar.gz` and checksums are being shipped to a separate helper for a full 751-board comparison of main, via-control and bundle-order at 192 jobs. No successful routing claim or commit yet.

### Completed 73-board diagnostic ablation (selected sample)

All 292 referee results are successful. This deliberately selects regression boards plus three controls, so its totals must not be generalized to the corpus. Every variant includes the via-name fix; no mask guard or dogleg changes.

| Candidate | Group | U better / worse | U delta | Copper DRC gainers | Copper delta | Mask gainers | Mask delta | CPU ratio |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| nominal | pcbench- KiCad (72) | 18 / 37 | +110 | 25 | +95 | 15 | +125 | 0.9919 |
| nominal | kicad- KiCad (1) | 1 / 0 | -8 | 1 | +1 | 0 | +0 | 0.9951 |
| smoothing | pcbench- KiCad (72) | 5 / 1 | -22 | 0 | -2 | 4 | +37 | 0.9923 |
| smoothing | kicad- KiCad (1) | 1 / 0 | -4 | 0 | +0 | 0 | +0 | 1.0759 |
| both | pcbench- KiCad (72) | 19 / 36 | +13 | 26 | +99 | 15 | +161 | 0.9189 |
| both | kicad- KiCad (1) | 1 / 0 | -7 | 1 | +1 | 0 | +0 | 0.9343 |

Deadline/capped-report variability applies; no timing improvement is claimed. Detailed exports and observations pulled locally and backed up to workbench. This experiment isolates mechanisms and is not an acceptance test for either change.

### Independent workbench via-name full run completed

`quality-kicad-via-name-01` completed all 751 cells with successful KiCad referees, no rescore needed. Compared with the existing e9 main workbench run: PCBench 54 U-improved / 4 regressed, 5321 → 4630 U (−691), 947 → 915 copper DRC (−32; 2 gainers), 13834 → 13961 reported mask (+127; 26 gainers), total CPU ratio 0.9375. Local 11: 1 improved / 0 regressed, 243 → 235 U (−8), 59 copper DRC unchanged, mask zero, CPU ratio 1.1313. Combined −699 U / −32 copper DRC / +127 reported mask.

The only completed connectivity regression remains Apple M0110 0 → 1. Other losses are deadline-limited Karabas G 93 → 94, nonSNES 175 → 182, decelerator 481 → 496. These independently reinforce connectivity benefit but also show why one timed run cannot establish speed or deadline-board quality. Mask differences require unchanged-SES comparison before attributing them to routing. Export details and comparison JSON are under `/tmp/quality-kicad-mask-pilot/quality-kicad-via-name-*`; PR24 report supplementation pending this follow-up audit.

Grouped-order full run `quality-epyc-bundle-order-01` is live, driver PID1184248, comparing main/via-control/bundle-order, 751 each / 2253 cells / 192 jobs. Frozen helper `/root/copperroute-bundle-order`, three-file checksums recorded in reports.

Workbench via-name SES audit: 635 identical pairs have zero U delta and +97 reported mask counts; 116 different pairs account for −699 U and +30 mask counts. The +127 total is not entirely a routing change, but actual mask regressions remain. Added this independent measurement and all four connectivity losses to PR24's body, preserving the original failed regression gate.

### Outlier-board register requested by user

Added `docs/outlier-boards.md` with three evidenced entries: programming-interface exposed copper artwork removed during stripping; AVR Fuser copper text omitted from DSN; relay-controller independent mask artwork. These remain included in all headline benchmarks. Ordinary routing regressions (Apple M0110, GB-CART256K-A) are not classified as outliers without independent evidence. The register separates confirmed input-fidelity issues from unusual but valid design features.

Programming-interface follow-up on KiCad 10.0.4 confirms 223 original unnetted F.Cu tracks and 223 F.Mask line objects. None of those tracks remain in the stripped input. Corrected occupancy audit excludes rule areas and uses filled zone polygons: original has 223 single-net apertures (unnetted copper), stripped has 223 empty apertures, combined candidate has 15 empty / 178 single-net / 30 multi-net. All counts are zero-clearance geometric contacts, not KiCad error counts. The 41-violation relay board has zero original unnetted tracks, so this stripping mechanism does not explain it. The preservation hypothesis still requires a controlled run; no dataset changes or exclusions have been made. Full corrected occupancy copied to the laptop beside prior artwork reports.

Grouped-order prototype default workspace suite completed: 2569 passed, 77 ignored, zero failed. Its three targeted grouping tests pass; full enabled routing behavior is under corpus evaluation, not implied by the default test-suite result.

### User's outlier-aware review policy applied

Audited open PRs and completed candidates against the three-entry register. PR22 changes scoring without routing changes on completed pairs; PR23 changes tooling; PR24's completed U regression is Apple M0110, which has no outlier evidence. None should be described as an outlier-concentrated routing regression. The combined mask+via candidate does have concentrated *mask* losses: programming-interface +34 and relay-controller +36 account for 70/87 positive mask-count changes (80.5%). It still has ordinary losses: 38 U-regressed and 26 copper-DRC-gaining boards outside the register. Full totals −1012 U / −57 copper / −4382 mask; excluding all three registered boards gives −1015 U / −82 copper / −4177 mask. No exclusion applied to headline results.

Preparing `copperroute-mask-routing-review`, branch `experiment/nominal-mask-routing-review`, based on PR22 commit36283c2. Includes via-name correction (PR24), nominal clearance, smoothing reserve, dogleg escape and pad-mask gap cap; excludes obsolete checker copy and failed-first prototype. Removed temporary print instrumentation, added input parse errors, retained unmatched-pad warnings, made sidecar gap-cap unconditional to match the measured enabled candidate. Native checker remains the separate base PR. First full suite exposed legacy geometry expectations; porting the previously reviewed explicit nominal deltas from the older combined experiment, retaining original JVM recordings and independent branch-behavior tests. The core-wrapper test now directly compares wrapped/unwrapped routing, and the fanout no-op fixture was subsequently repaired by extending its target to (20,20), keeping the obstacle inside the nominal query envelope while preserving the same positive sub-grid movement. Raising its clearance to116 was tested and failed (zero movement), so that attempted fixture change was discarded.

An immutable 12-file snapshot of the packaged production code is under `/root/copperroute-mask-review`; full run `quality-epyc-mask-review-01`, driver PID3096545, compares main, original mask-prototype and mask-review, 751 each at192jobs. Local test-only adaptations continue independently; final production equivalence to this snapshot must be checked before committing. New PR not yet open.

Grouped-order full run completed all2253 KiCad cells. Versus via-control: PC740 U4694→4770 (+76), 54better/51worse; copper915→963 (+48),22gainers; mask15190→15113 (−77),34gainers; CPUratio.9693. Local11 U241→231 (−10),2better/2worse; copper59→58 (−1),1gainer; mask0; CPU1.0267. Total+66U/+47copper, so no acceptance claim. KarabasC alone contributes+166U, both timeout; among both-completed pairs totalUdelta is+3. NRC2016RAM improves1→0U with0copper; GBCART stays3U and improves2→0copper. Registered outliers account for only+1U, so the outlier criterion does not explain this result. Keep experimental pending the Karabas regression investigation; do not reject solely on aggregate timeout delta.

### Packaged mask-routing candidate ready for outlier-aware PR review

Full run quality-epyc-mask-review-01:751 boards per candidate, all KiCad referees successful after68 failed-referee cells were repaired on unchanged SES. PC740:140 connectivity improvements/38 regressions,5282→4132 U,933→885 copper DRC (26 gainers),15193→10949 mask (11 gainers),CPUratio0.8730. Local11:3 improvements/0 regressions,241→226 U,59→57 copper (1 gainer),mask0,CPUratio0.9189. Total −1165 U/−50 copper/−4244 mask. Gate fails268 quality losses including other score components. Artwork programming-interface+32 and relay+36 account for68/112 added mask reports; previous70/87 remains recorded as a separate measurement. No outlier exclusions. Prototype/review SES identical on720/720 both-completed pairs,727/751 overall. All differences involve at least one timeout. Production sources match frozen snapshot after rustfmt, excluding only changed test expectations. Workspace2579 passed/77 ignored/zero failures; both repaired independent regression tests pass. Detailed report, source-equivalence proof, raw bench pr-summary and full per-board deltas included in review branch. Full raw results and final details verified backed up on workbench.

Controlled programming-interface preservation pilot completed all3 KiCad referees: stripped7U/0Cu/32mask; identical SES with223 original copper artwork tracks restored7U/26Cu/0mask; supplying223 actual copper-outline obstacles before routing plus preserved referee3U/0Cu/0mask. Confirms this input-loss mechanism can be resolved for this board. No corpus changes or global improvement claim. Results and scripts retained in PR25 artifacts; independent relay mechanism remains open.

### Coherent corridor review preparation

Consolidated full751-board control/main comparisons, raw generated bench pr-summary and matched600-second diagnostic details in docs/corridor-routing-report.md and routing-quality-artifacts/corridor-coherent. Report separates incremental−241U/0Cu/−113reportedmask from inherited PR25 gains, splits refereed groups and deadline effects, discloses memory and ordinary regressions. All five implementation hashes match frozen server files. Preparing opt-in draft review; no claim of default activation readiness. PR body /tmp/quality-corridor-pr-body.md includes benchmark summary. Fresh full workspace session20112 running at /tmp/quality-corridor-review-workspace.log before commit; do not compile another worktree into shared target until it exits. No PR opened yet.

Fresh review workspace completed2584passed77ignored0failed, including doctests (/tmp/quality-corridor-review-workspace.log, session20112 exited0). All source files still match frozen server hashes. Added explicit native-web limitation: its per-pad component representation loses component-pair grouping; enabled measurements use DSN identities and inherited mask sidecars. Preparing commit/draft PR with complete measured trade, no default activation.

Correction to fresh review validation: session20112 exited0 but ran2581 tests including board-minimum-specific names and no corridor tests. Shared-target fingerprints reused the other worktree's binary despite the correct working directory. This is not corridor validation. Cleaned the shared target entirely and started explicit --manifest-path full suite33864, log /tmp/quality-corridor-review-clean-workspace.log. Do not build another worktree there while it runs. rustfmt also reordered control/corridor module declarations; report distinguishes this formatting-only difference from frozen source hashes. No commit/PR has occurred.

Original-reference check: motorized-opener ground_truth has0wirelength/0vias; saved raw-drc has37clearance violations and149unconnected entries. Thus this fixture is not a completed human-routing reference. Its ground_truth layers=1 conflicts with DSN listing F.Cu and B.Cu; do not assume a one-layer routing problem from that stale field. All candidate regressions remain counted; no outlier exemption inferred. Azalea original is connected,3183.4078mm/176vias/0routing DRC, so an actual human solution exists there.

Clean explicit-manifest review suite33864 exited0:2584passed77ignored0failed, all five corridor tests present and no board-minimum tests. Compilation log confirms all workspace crates built from this worktree. Corrected report/body to final verified results.
### Pad-local copper floors: native and metadata implementation

Observed named-pad native and DSN metadata tests failing without the change, then passing restored. Copper floors are independent of the mask gap cap and round upward; existing larger clearances remain. Browser pad/footprint inheritance uses KiCad's format-version zero semantics. Initial workspace2585passed/77ignored; corpus census then revealed valid negative local overrides, so withdrew the incorrect rejection assumption, observed replacement tests fail, and fixed native/browser handling. All49 web tests and three native clearance tests pass. Full updated workspace suite is live at /tmp/quality-pad-local-workspace-final.log (session25710); no commit.

Census using KiCad10.0.4 completed751boards,78with overrides,1557annotated source pads,zero unmatched. Three boards use coincident pads: AVR-ISP pogo, soil-moisture-sensor and USB FT245R. Sidecar generation assigns their maximum local clearance to coincident records; conservative scope and possible added routing difficulty are documented. Old metadata remains frozen. New metadata reports/pad-local-copper-metadata-03, census pulled locally.

Frozen source uploaded from c461555 archive plus production diff; all three relevant Rust file hashes match local. Driver501303 queues release build and quality-epyc-pad-copper-pilot-01 after corridor regression driver494648 exits. Five matched boards include USB FT245R to measure duplicate-pad effects. No new-code corpus quality claim yet.

Updated full workspace completed successfully:2,584 passed,77 ignored,zero failures including doctests; session25710 exited0. Log /tmp/quality-pad-local-workspace-final.log. No concurrent shared-target compilation occurred. Frozen remote hashes still match. Pilot driver501303 remains queued behind corridor regression driver494648 (confirmed live); no rerun or restart.

### Uncapped pad copper pilot completed; full corpus queued

quality-epyc-pad-copper-pilot-01,5PCBench boards×2candidates,all completed and KiCadok.49→55U (+6),0improved/2regressed;59→13Cu (−46),2improved/1gainer;412→383reportedmask (−29),1improved/0gainers. CPU428.13→508.01s ratio1.18658; medianRSS26.7→27.1MB/max66.3→375.8MB. APM and USB FT245R unchanged quality. Minimal_node1→3U/2→0Cu; azalea37→41U/45→0Cu; BLDC11Uboth/9→10Cu and228→199reportedmask. Memory and CPU increases are material pilot costs; no claim of speed or merge readiness.

Full quality-epyc-pad-copper-full-01 queued with frozen main,control,pad-copper751each/192jobs/10passes/300s/one thread. Driver523176 waits for board-minimum pilot518369 to finish, preventing build overlap. Integrated unchanged-SES referee repair and detailed export; final pilot details verified on workbench and pulled locally. Local full workspace already passed2584/77ignored/0fail.

### BLDC pad-local cost and width audit

Pilot BLDC RSS56.3→375.8MB and CPU165.06→243.19s. Final item count1818→1793, tracks773→754, segments1233→1196, vias58→52: retaining more final routing does not explain the memory rise. Search/temporary allocation instrumentation is needed; no memory fix inferred. Both candidates emit repeated unmatched thermal-pad metadata warnings (Q2.5/Q4.5), so the source-pad census's zero unmatched records does not prove every DSN pin matched. Census only validates metadata against original pads. The snapshot's matching behavior is unchanged between candidates; do not claim complete DSN rule hydration.

BLDC's9→10 copper violations are entirely track_width; both reports say minimum0.2500mm, actual0.2498mm. Neither pilot candidate has clearance violations on BLDC. Pin::get_trace_neckdown_halfwidth subtracts one board unit from half the pad width, matching a possible two-unit total-width undershoot. This is a lead, not a proven per-trace attribution. Before changing it, instrument/reproduce the affected insertion and consider adding a full-pad-width attempt ahead of the existing margin-shrunk fallback, preserving options and JVM recordings with explicit divergence tests. Frozen corpus candidates unchanged.

### Full uncapped pad-clearance comparison completed

quality-epyc-pad-copper-full-01 all2253cells KiCadok; final details pulled locally and verified on workbench. Versus control PCBench740:4511→4222U (−289),17improved/3regressed;881→766Cu (−115),14improved/5gainers;11019→10984reportedmask (−35),6improved/9gainers;CPUratio0.94991;completed710→716,connected588both;RSSmedian12.1both/max370.9→372.3MB. Local11 unchanged U226/Cu57/mask0,completed10both/connected8both;CPUratio0.99767;RSSmedian13.5→10.6/max76.1→78.3.

Both-completed719 pairs contribute+4U/−119Cu/−48reportedmask;deadline-affected32 contribute−293U/+4Cu/+13mask. Thus overall connectivity gain is deadline-sensitive; no reliable speed or deterministic connection-gain claim. Three ordinary U regressions: minimal_node+2U/−2Cu,azalea+4U/−45Cu,mppt-2420-hpx+2U/0Cu. Five copper gainers:BLDC+1,Brushless_ESC+1,decelerator+2,oled-bmp280-touch+2,real-time-chess+2. Need inspect mppt and Cu gainers, plus same-SES/metadata attribution before deciding merge trade. No outlier-concentration claim.

Started clean explicit-manifest full workspace before review; /tmp/quality-pad-copper-review-clean-workspace.log. Shared target cleaned to avoid known stale cross-worktree artifacts. No concurrent local compile. Board-minimum full871369 now active after this run finished.

Attribution audit: annotated71both-completed+4U/−119Cu/0mask;annotated7deadline−54U/+1Cu/+18mask;unannotated648both-completed0U/0Cu/−48mask;unannotated25deadline−239U/+3Cu/−5mask. All648unannotatedcompletedSES hashes match exactly. Mask−48 is not attributable to routing. First control uses frozen corridor-disabled binary, candidate uses pad-copper binary, so compilation effects and deadline noise cannot be separated. Queued same-binary old/new metadata run quality-epyc-pad-copper-matched-01 driver3678230 after active board-minimum871369. All751×3again,192jobs, no new build.

MPPT regression7→9U/0Cuboth is associated with explicit700µm copper floors on its two fiducials FID1/FID2. Original connected/0routing DRC/3695.0475mm/274vias. No outlier exemption or board-specific bypass inferred.

Clean explicit-manifest full workspace18544 completed:2584passed77ignored0failed including doctests. No shared-target artifact reuse; all crates rebuilt. Same-binary full repeat3678230 still queued behind active board-minimum871369. No pad-clearance PR yet pending the attribution repeat and honest completed-board trade report.

### OLED outline regression attribution

Original oled-bmp280-touch has 15 invalid_outline errors (disconnected Edge.Cuts segments/arcs), zero routing DRC and zero unconnected items; 419mm tracks/21vias. Exported DSN has a single rectangular boundary and zero keepouts, omitting its circular cutouts. Both new pad-local copper_edge_clearance errors hit the same original circle UUID578ea680 at (122.555,105.156)mm with OLED_RST tracks. Registered as confirmed original-outline/input-fidelity outlier; no exclusion or claim of repaired routing. Audit script and original/candidate reports saved locally and on the server. Together with Brushless text, this explains 3 of the first pad-full run's 8 added copper violations across five boards; it is not a majority and ordinary connectivity regressions remain. Same-binary full repeat still active (driver3678230), neckdown pilot89138 waiting.

### Same-binary pad clearance repeat complete

quality-epyc-pad-copper-matched-01: all 2,253 KiCad referee cells successful. Same frozen executable, old/new metadata. PC740: 4503→4044 U (−459;17 improved/3 regressed), 885→767 copper (−118;14 improved/4 gainers), 10967→11006 mask (+39;10 gainers), CPU ratio0.949785; completed710→716, connected588→589, medianRSS12.1both/max367.9→374.3MB. Local11 unchanged U226/Cu57/mask0, completed10/connected8both, CPU0.993785, RSSmedian13.6both/max66→79.1MB.

719both-completed pairs repeat +4 U/−119 copper, reported mask−7. 32deadline-affected contribute−463 U/+1 copper/+46mask. Ordinary regressions repeat minimal_node+2U/−2Cu,azalea+4U/−45Cu,MPPT+2U/0Cu. Copper gainers Brushless+1 and OLED+2 are evidenced input outliers; BLDC+1 and chess+1 are deadline-affected. Outliers account for3/5addedCu this repeat versus3/8first full, but do not explain the ordinary connectivity regressions. No reliable459connection orspeed claim. Details pulled locally; attribution hashes and main report still to prepare before PR.

Review preparation: same-binary completed hash audit719pairs/685identical;34changed+4U/−119Cu/0mask. All−7reportedmask lies on identical outputs. Rechecked frozen production hashes and clean workspace2584passed/77ignored/0failed including five new named tests. Main stille9d10c2. Generated main report and incremental reports retained; preparing draft PR with ordinary connectivity losses and two input outliers explicitly separated.
### Board-wide copper minimum: controlled input experiment

Snappi-zero's KiCad project minimum is203µm; DSN default203.2µm, smd_smd50.8µm and one net class152.4µm. Controlled run quality-epyc-snappi-minimum-01 uses unchanged frozen corridor binary, with only two DSN values floored to203µm. All three cells completed and KiCad-refereed: mask-control0U/3Cu/5vias/1.91CPU; coherent0U/10Cu/3vias/1.78CPU; corrected-input coherent0U/0Cu/3vias/2.63CPU. All retain22 courtyard reports, separate from copper/mask routing metrics. Evidence supports omitted board minimum, not an outlier; no global quality claim. Details pulled locally, remote input and changes under reports/snappi-board-minimum-study.

Created isolated fix/kicad-board-minimum-clearance at c461555. board_prep::raise_to_project_minimums already applies mask, edge and hole rules, but ignores constraints.min_clearance. Added failing-first test: positive project copper minimum must raise lower matrix classes, preserve higher values/null class and be idempotent. It fails at the raise call as expected (/tmp/quality-board-minimum-red.log, session59835 exited101). No production change yet. Before implementation, distinguish dedicated board-edge/hole/keepout rules from copper-pair clearances and ensure search-tree compensation is rebuilt when matrix values change. DSN benchmark must supply the original project's minimum to exercise the fix; optional CLI project import currently stores constraints for DRC but does not enforce copper minimum in routing.

Implemented copper minimum in board preparation. Collect clearance classes used by existing copper items plus configured future trace/pin/SMD/via/area classes; raise only copper-class pairs, preserving class0 and dedicated classes not used for copper. Existing shared copper/keepout classes can still conservatively tighten their keepouts; this is a matrix-model limitation to quantify, not a claim of exact KiCad override parity. Uses searchtree::clearance_value_changed after matrix updates. Scoped matrix test observed red then green; cached compensated pin shape test passes and fails with the refresh disabled (equal old/new shapes), so cache invalidation has direct coverage. Full workspace3713 running, /tmp/quality-board-minimum-workspace.log; no commit or production-corpus result.

Census across751board projects:694 positive minimums, all694 have some lower DSN value due to SMD-specific rules. Only5boards have lower untyped clearances: CATs-Eurosynth_LFO_Main(.203mm), domotics_base-board-arranged(.33), mini_ice40(.18), snappi-zero(.203), wavegen_rev3(.19). Do not treat694 as694 missing net-class floors. Diagnostic census pulled locally. A separate pilot should feed identical minimal project metadata to control and candidate, isolating copper minimum enforcement from unrelated project rules.

Frozen board-minimum source uploaded from c461555 archive plus diff; both Rust SHA256s match local. Driver518369 built after pad pilot501303 exited and now runs6boards×2candidates (five below-minimum untyped DSN cases plus minimal_node). Both get identical minimal project metadata containing only min_clearance and original mask sidecars, isolating this change from other project rules. quality-epyc-board-minimum-pilot-01 at10passes/300seconds/one thread/192capacity. Full workspace3713 still live; no commit.

Full workspace completed successfully:2,581 passed,77 ignored,zero failures including doctests (/tmp/quality-board-minimum-workspace.log, session3713 exited0). Frozen remote source hashes match. Six-board production-code pilot518369 remains live; full pad run523176 waits for its completion. No commit or corpus-wide board-minimum conclusion yet.

### Board-minimum production pilot completed

quality-epyc-board-minimum-pilot-01 completed12cells with successful KiCad referees. SixPCBench boards:19→18U (−1;1improved/0regressed),36→5Cu (−31;3improved/0gainers),208→207reportedmask (−1;0gainers). CPU458.03→482.53s ratio1.05349;5completedboth/3connectedboth; medianRSS22.4→23.9MB/max99.8→98.4MB. Snappi3→0Cu,mini_ice4027→0Cu,LFO4→3U/1→0Cu. Wavegen remains14U and timed out; other controls unchanged quality. Details and comparison pulled locally.

Queued full quality-epyc-board-minimum-full-01, driver871369, behind currently routing pad-copper-full driver523176.751each main/control/board-minimum,192jobs/10passes/300s/one thread; same minimal project metadata for control/candidate; frozen binaries, integrated referee repairs/export. No source changes after pilot. Corpus-wide conclusion pending.

### Full board-minimum validation and review

quality-epyc-board-minimum-full-01 completed 2,253 successful KiCad-refereed cells, 751 per main/control/candidate. Against control: PCBench −336 U (16 improved/0 regressed), −29 copper DRC (4 improved/1 gainer), −1 reported mask (6 gainers), CPU ratio 0.9444; local 11 unchanged quality, CPU ratio 0.9998. Of 720 both-completed pairs, 717 SES files are identical. Only Snappi, mini_ice40 and LFO change: jointly −1 U/−31 copper with no completed connectivity/copper regressions. Their +10 reported mask difference is entirely on identical SES files. Deadline-affected pairs account for −335 U/+2 copper; no reliable speed or 336-connection causal claim. Main comparison includes PR25 and fails the aggregate gate with 266 quality losses; full generated summary retained. Main rechecked at e9d10c2. Results verified mirrored to workbench and copied locally.

Clean explicit-manifest precommit suite50262 exited0: 2,581 passed/77 ignored/zero failures including doctests. Correct worktree compilation paths and both new tests verified. Report and control/main comparisons included. These are ordinary project rules, not an outlier exemption.

### Combined corridor and project-minimum full result

quality-epyc-corridor-minimum-full-01 completed2253successfulKiCadcells. PC740control→enabled:−172U(44better/13worse),−3Cu(13gainers),−64reportedmask(12gainers),CPU0.940064;local11+4U(2worse),+1Cu(1gainer),0mask,CPU1.007780.721both-completed−46U/−3Cu/−77mask;495identicalSES accountfor−13mask,226changed−46U/−3Cu/−64mask.30deadlinepairs−122U/+1Cu/+13mask. Snappi0U/0Cuboth, fixing prior3→10Cu corridor regression. Ordinaryazalea+5U/+6Cu andmotorized+3U/+1Cu remain. Full main summary fails262qualitylosses and includesdependencies. Results locally andworkbench backedup.

Merging PR28 into PR27 review branch with --no-commit, preserving both histories; only running-log conflict resolved by retaining both sets of entries. Seven production files identical to measured combined snapshot. Freshcleanreviewworkspace80300 running; no merge commit until exit0. New report/artifacts prepared; PR27 will stack on PR28 after validation. Fullneckdown2583304 nowactive.

Freshcleanreviewworkspace80300 exited0:2586passed/77ignored/0failed including doctests. Correct worktree compilation paths and seven expected tests verified; no stale target reuse. Ready to commit the PR28 merge into PR27, preserving both histories and the failing main gate disclosure.
### Explicit image identity revisited on merged main

Reproduced the previously logged Saiboard import bug on current code (not a newly discovered mechanism): U2/U18 receive 1.475×0.6mm rounded SOT23 geometry intended for Q3/Q5; Q3/Q5 receive0.9×0.8mm rectangular geometry instead. Q3pad1 imported(29.194,98.618)mm vsoriginal(29.1315,98.618);U2pad2(50.95,42.2875)vsoriginal(50.95,42.35). Imported copper rule is200µm, so rule absence does not explain these mismatches. NativeDSN+SES+projectDRC returns0violations whileKiCadboundaryoutput26Cu errors. Diagnosticexample removed from PR29 worktree and retained as artifact.

Created experiment/image-identity-main atd84ac9e. Replayed earlier library_scope test first:13passed1failed, explicitnames dedup3→2. Applied only two earlier production files (library parser preserve names, package lookup prefer exact opposite-side beforefallback);14library tests nowpass. Fullworkspace36904 running in freshly cleaned shared target; no legacy recordings or expected outputs updated. Earlier reviewed patch includes test accommodations and references a missing preserved-image-tree-order.txt artifact; those accommodations were not applied. Reassess current full-suite differences before any commit. Historical fullrun had mixed+71U/−25Cu and deadline failures, retained as evidence.

Prepared frozen image snapshot and full quality-epyc-image-main-01, main/images751each,192jobs10passes300s1thread, oldmask metadata andsame minimalprojectrules onboth. Waits behind duplicate-referencepilot4106410 andpadfull3631690; compiler runs only afterbothcomplete. Currentmainbinary reused fromnewpadfull. Integratedrefereerepair/export; fullacceptancepending.

### Unchanged-session importer attribution

Using fresh image-worktree CLI from the current clean-target build, imported the identical Saiboard boundary SES into corrected image geometry. Native DRC now detects13clearance/3shorts plus17dangling tracks, compared with0reports underwronggeometry. Example HallOut2 pin versus P trace actual0.1243mm matchesKiCad,expected0.2mm. KiCad has19clearance/7shorts; counts are not equivalent because native uses different item aggregation. No reroute or independentqualitygain claim fromthisaudit. Logs/report saved. Current fullworkspace shows five parity-test targets failing (overrides,net_incompletes,unconnected,dsn_reader,parity_ses), stillrunning; originalrecordings untouched. Frozenimagefull driver924409 queued behindpadfull3631690 andduplicatepilot4106410.

Image workspace36904 completed2577passed/5failed/77ignored. Temporary env-gated override transcript audit removes only tree_order lines from both assertions; every other original clearance-state field passes across all cases/stages. Recorded all changed tree-order rows as evidence; restored original overrides.rs byte-for-byte. No expectation or original JVM recording changed. Other four failing targets remain under review; source frozen for queued corpus experiment.

### Image parity review

Applied prior explicit accommodations to all five failing targets after reviewing current differences. Original JVM files untouched. SES test now checks every non-placement byte againstJVM, every placement field againstJVM, and exact image names againstsourceDSN (retains knownCyrilliclexerexception). Relay reader keeps all wiring/vias while explicitly updating named image/outline-order expectations. BBD connectivitymaps three IDs; new independent assertions confirm873=U102-17,867=U102-23,935=C2-1. Override audit yielded exactlyfour consistenttreeorders across12changedstagecases; wrote separateRusttreeexpectations andpreservedall originalnon-orderfields. All5targetedtestbinariespassed; pinidentitycheckpassed. Fullreviewworkspace nowrunning /tmp/quality-image-main-reviewed-workspace.log. No productionchangesafterfrozenimagecandidate, no corpusacceptanceyet.

### Reviewed image-import suite completed

Fresh reviewed workspace session21977 exited0. All167 test summaries total2582passed/77ignored/0failed, including doctests. The five original Java-parity failures were reviewed explicitly; original JVM recording files remain unchanged and named Rust expectations account for preserved image identity. Physical identities of the three renumbered pins are independently asserted. Full corpus driver924409 remains live and routing quality-epyc-image-main-01, exact main d84ac9e versus image correction,751boards per side. No corpus success claim yet.

### Image identity full validation against merged main

quality-epyc-image-main-01 finished all1502KiCadok. PCBench740:−346U(25better/6worse),−49Cu(11better/3gainers),−15reportedmask(11better/7gainers),CPU0.93829,completed710→718,connected588→590,RSSmedian12.1both/max369.4→373.9MB. Local11 unchangedquality/completion/connected,CPU1.00626,RSSmedian12.1both/max71.7→70.3MB.

720bothcompleted:−11U/−48Cu/−14reportedmask. Of these185SESidentical give0U0Cu/+4mask,535SESdiffer give−11U/−48Cu/−18mask. SES image names intentionally change, so a changed SES is not proof of changed wire geometry. Five ordinary completedboards loseconnections:serial_gw+2,minimal_node+2/−2Cu,Pi1541io+3,teensy-fx+1,esp32-ethernet+1/+1Cu. Balena-rover adds1Cu without connectionchange. Chess+3Cu andbms+1U aredeadlineaffected. Saiboard8x3 retains0U and loses17Cu. Historical oldbaseline image experiment was negative; current main's full result now supports preparing review, with regressions explicit. No346causalconnection or6percent speedclaim. Detailed results copied locally/workbench; generated report preparation underway.

Image report prepared against reverified main d84ac9e. Original/referee regression audit confirms both completed copper gainers involve unimported local NPTH clearance rules: esp32 1.650mm, balena1.725mm; both originals0routingDRC/0unconnected. These remain ordinary regressions. Source hashes match frozen candidate; reviewed clean suite2582passed77ignored0failed. Generated gatefails19qualitylosses. Preparing a separate draft PR with the +11connection/−48Cu completed-board benefit and all losses explicit; combined local-rule/image behavior not measured.
### PR29 integration with merged main

Previous goal turn completed the authorized merge batch (progress). Rechecked main d84ac9e and merged it into fix/kicad-pad-local-clearance without committing. Only docs/routing-quality-log.md conflicted; both experiment histories retained. Production integration needs no conflict edits. Fresh clean workspace24668 running at /tmp/quality-pad-main-integration-workspace.log; web49passed after correcting an initial npm invocation from the repository root.

Server idle after boundary run, launched driver3631690 /root/quality-epyc-pad-post-main.sh. It builds separate frozen snapshots of exact main d84ac9e and main plus the PR29 code, then runs quality-epyc-pad-post-main-01,751boards each main/control/pad-copper,192jobs10passes300s1thread. Main uses old pad metadata; control/newmetadata use the identical candidate executable. All three use identical minimal project minimum-clearance files, ensuring the newly merged project floor is exercised. New metadata is the already audited pad-local-copper-metadata-03 census. Serialized builds precede routing; integrated unchanged-SES referee repairs and detail exports. No new corpus results or merge claim.

### Duplicate component metadata mismatch reproduced

Fresh merged-main PR29 workspace24668 exits0:2586passed/77ignored/0failed including doctests,web49passed. No commit while new full validation is pending. Main/control/candidate full driver3631690 remains active.

Boundary-regression inspection: RJW CPU board adds4hole-clearance errors against W1 NPTH1.65mm;96boards Sensors adds7Cu errors against1mm mounting pads;saiboard adds9Cu with no local overrides on inspected pads. Originals all0routingDRC/0U, ordinary designs. Groundtruth respectively7912.35mm318vias,3968.10mm140vias,5925.26mm148vias. Pad metadata includes the large rules, but 96boards old pad-local run logs three unmatched REF**.1 records. Raw has four REF** footprints; stripped renames three REF**_2/_3/_4 while preserving UUIDs/positions. Exact component matching prevents those three rules from applying. Earlier source-census zero-unmatched did not prove loader hydration; this is now a concrete counterexample, not an outlier exemption.

Prepared diagnostic metadata changing only the three names via UUID correspondence and exact positions. Queued quality-epyc-duplicate-pad-pilot-01 behind currentfull;same frozen newpadbinary andprojectmetadata,old/fixedpadnames,oneboard10passes300s. No production loader relaxation or change to ongoing full inputs. Audit/script retained locally andserverreports.

### Post-merge pad-clearance validation and duplicate-reference pilot

quality-epyc-pad-post-main-01 completed all2253cells with KiCad statusok. Main rechecked d84ac9e. PCBench versusmain −302U(16better/3worse),−117Cu(4gainers),−14reportedmask(8gainers),CPU0.94990; local qualityunchanged,CPU1.00399. Samebinary PC −278U/−117Cu/−74mask,CPU0.95004; localCPU0.99715. Crucially34changed completedoutputs repeat +4U/−119Cu/0mask;685identical completedcontrolpairs account−98reportedmask. Main/control719completedSESallidentical. No causal302connection or5percent speedclaim. Generated gatefails13qualitylosses. Report and full comparisons refreshed; local/workbench backups verified. Fresh integration suite2586passed77ignored0failed;web49passed.

Duplicate-reference pilot quality-epyc-duplicate-pad-pilot-01 finished bothKiCadok:96boards Sensors0U13Cu→0U0Cu,113→105vias,3747.9467→3834.5618mm,CPU153.67→153.08s,RSS57.6→54.7MB. OnlythreeUUID-mapped componentnameschanged;samebinary/rules. This is an ordinaryboard and a concrete metadata propagation fix, not an outlier exemption. Pilot details copied locally; production generator correction and corpus validation still pending.

### Duplicate-reference correction scope and conservative mapping

Previous goal turn made progress: PR29 integration b857afc pushed and report updated against d84ac9e. The full source-reference census now completes751boards, flags renamed records on166boards;17include copper floors. These are candidate matches, not validated renames:298records have multiple destination names, and legacy missing/duplicate footprint UUIDs require independent physical checks. Raw census retained.

Added pure reference_mapping helper and five failing-first tests. Initial no-op produced four expected failures (duplicate reference remains wrong, ambiguity not rejected, missing UUID not rejected, unmatched not reported); implementation now5passed. No original records are mutated. The corpus adapter requires unique UUIDs on both boards, equal footprint position/orientation/layer and matching pad number/position/size/shape/layers. It preserves and reports unresolved metadata instead of guessing. Diagnostic generator driver2582866 launched to create a separate metadata-04 directory; no active benchmark inputs changed. Review generated diff and known Sensors pilot equality before any routing run. Full Rust suite required again before committing these additions.

The initial geometry adapter incorrectly compared Python SWIG LSET wrappers with ==, which compares wrapper identity rather than layer contents. Sensors consequently produced0changes/387unresolved, failing the known pilot check before any routing. A direct probe showed equal pad position/size/shape and identical [0,1,13] layer lists but wrapper equalityfalse. Corrected comparison uses list(LSET.Seq()). Preserved metadata-04 as a rejected diagnostic, and launched a new separately named metadata-05 generation with verified-census output. No routing result used the rejected metadata.

### Verified reference metadata full run launched

Corrected layer-list geometry audit completed751boards. Validation proves only component names change, all other per-record fields/counts remain, and Sensors metadata exactly equals the successful13→0Cu pilot. Full quality-epyc-reference-remap-full-01 driver2635950 launched after image run completed,751each main/control/remapped,192jobs/1thread/10passes/300s. Main frozen d84ac9e; control/remapped same frozen PR29binary withmetadata03/05, identicalprojectfloorfiles. No concurrent release build. All referee environment set; integrated rescoring/export/details. Inputs hashed before routing.

### Full reference remap result

quality-epyc-reference-remap-full-01 completed2253KiCadok. Against samebinary padcontrol, PC740−360U(15better/0worse),−24Cu(5better/1gainer),−365reportedmask(10better/9gainers),CPU0.938956;local11unchangedquality,CPU0.994062. Completed719pairs:708identical0U0Cu/−404reportedmask;11changed−3U/−21Cu/−7mask. Only two completedboards change U/Cu: Sensors0U/−13Cu;OpenHardwareExG Shield−3U/−8Cu. No completedconnectivity orcopperloss;chess+3Cu isdeadlineaffected. No360connectionor6percent speedclaim.

Againstmain d84ac9e:PC−384U(15better/4worse),−139Cu(16better/5gainers),−339mask(9gainers),CPU0.94485;localqualityunchangedCPU1.001802. Completed718pairs677identical0U0Cu/−396mask and41changed+1U/−140Cu/−7mask. Thus metadata correction improves PR29's repeatable trade from+4U/−119Cu to+1U/−140Cu. Main/control again reproduce+4U/−119Cu on34changed completedoutputs. Results pulledlocally;workbenchcopy underway. Referee variability onidenticaloutput dominates reportedmaskdifferences. Next: integrate verified metadata mapping into reproducible generator, refreshPR29 report, and inspect combinedimagepilot beforemerging.

### Reference mapping integrated into generator

Refactored the exact measured physical-match adapter into remap_board_references in reference_mapping.py and invoked it after source copper annotation in generate-metadata.py. Unresolved names and rejected geometry remain explicitly included in the census. Five mapping tests pass; scripts compile. Launched reviewed generator driver372050 into a separate output directory to compare all751files with the measured metadata05; no rerouting needed if exact equivalence is established. Validation still pending while generator runs; no commit.

Reviewed generator verification completed: all751generated board JSONs equal the measured metadata05 exactly (zero mismatches). This validates the refactor against the actual full-run inputs. Source production Rust remains unchanged from the measured PR29 build.

### Combined pilot and updated pad report

Eight-board quality-epyc-image-pad-pilot-01 completed32KiCadok,allroutesCOMPLETED. Totals main9U63Cu,images18U46Cu,pads11U20Cu,combined15U5Cu. Selectedregressionboards,notrepresentativecorpus. Combined removesserial_gw image-only+2U and retainsSaiboard17→0Cu/Sensors17→0Cu, but minimal_node1→4U andPi1541io6→7U remain. Esp32main0U1Cu→combined1U2Cu andbalena0U2Cu→0U3Cu persist. Their unmatched NPTHmetadata is a distinct representation limitation: DSN mountingholes are keepouts,while metadata loader iteratespins only. Existing floor values therefore were not enough; do not claimlocalrulesfixtheseerrors.

Queued full quality-epyc-image-pad-full-01 driver389871 behindcorridorterminalpilot382532:main,pads,combined751each,samefrozenpilotbinariesandmetadata,192jobs10passes300s1thread. No builds overlapactive measurements. Resultsbackups active. PR29finalreport updatedwithreference-remapfullcomparisons,generatorverified751exactinputs,mechanicalpadlimitation. Fresh padprecommit workspace41293 running before commit; generated report extracted,gatefailure retained.

Fresh pad-remap precommit workspace41293 exited0:2586passed77ignored0failed including doctests. Allfive mappingtests pass. Refactored helper formatting preserves the exact Python AST; all751 generatedinputs already verified identical to measuredmetadata05. Gitdiff confirms no crates/webchanges from measuredb857afc. Ready to publish the incremental reference mapping and updated PR29 report: completed versusmain+1U/−140Cu; remaining NPTHkeepout gap explicit.

### PR31 integration and mechanical-hole rule diagnosis

Merged origin/main028f0a5 into image PR31 worktree withoutcommit. Onlyroutinglog conflict; retained both histories. Alltrackedcrates/web/manifests byteequalthe tested image-pad integration/frozencombinedsnapshot. Fresh fullworkspace11159running,sharedtarget nowimagePR31. No sourceconflictedits. Fullcombinedrun389871confirmedlive.

KiCadNPTHaudit shows balena Noname1-4 each2.75mm drill/size,1.725mm explicitlocalclearance; DSNcirclekeepouts diameter3.25mm alreadyinclude0.25mm radialpadding. Esp32 REF** variants each3.3mm drill/size,1.65mm explicitlocalclearance. Existing loadermatchespins only,while these arecomponentObstacleAreas,explainingremaining unmatchedrules. A future circlekeepout rule must account for existing geometricpadding: required extra rowfloor=max(drill_radius+local_clearance−keepout_radius,0), preservinghigherexistingrows. This is a geometric hypothesis to test,notyet implemented; actual importedclass/clearance stillneedsinspection. It does not revive the rejectedglobal250µm hole-clearance hypothesis. Auditretained.

PR31 post-main integration workspace11159 exits0:2587passed77ignored0failed includingdoctests;alltrackedsourceequalpreviouscombinedsnapshot. Mergecommit remains pending full benchmark assessment.

### Full image/pad interaction completed

quality-epyc-image-pad-full-01 finished2253KiCadok. Againstpadcontrol (productioncode/inputsequivalentnewmain028f0a5):PC740−288U(25better/4worse),−46Cu(10better/3gainers),−766reportedmask(12better/7gainers),CPU0.934578;local11unchangedqualityCPU1.000818. Completed720pairs186identical0U0Cu/−723reportedmask and534different−15U/−46Cu/−49mask. Changed SES includesimageplacementnames,notnecessarilychangedwires. No288connection/6.5percent speed/massivemaskgainclaim. Againstoldmaind84completed718pairs−14U/−186Cu. Oldmain/padcontrol againrepeats+1U/−140Cu on41changedcompletedoutputs. Positivecombinedcompletedtrade supports PR31review/mergeafterupdatedreport,while ordinaryregressions remainexplicit.

Details pulledlocally;workbenchbackup underway. SourcePR31postmainworkspacealready2587passed77ignored0failed. Fourboardlocalholepilotdriver2320983 launched afterfullrun; noholequalityclaimyet. Itsinputvalidation andfrozenbinarybuildserialized; sixfocusedtests pass.

PR31 finalmain audit:599baselinefiles matchcurrentmain028f0a5 and600candidatefiles matchfrozencombinedsnapshot,includingcratesource/testdata/manifests. Initialaudit mistakenly usedcandidatefilelistforbaseline and failed on the intentionallynew Rusttreeorderfile; corrected separatefilelists,completeverificationpasses. Generatedreport17qualitylosses retained. Updatedmerge rationale completed−15U/−46Cu; fourordinary+1U boards,twocompleted+1Cugainers explicit. Freshsuite2587passed77ignored0failed verified. Publishing integratedPR31 forauthorizedmerge.

### Corridor terminal coverage diagnostic

Read-only standalone diagnostic reuses PR27's exact for_net/choose_chain implementation, with the current corrected-image parser from the integration worktree. It inspects guided-net terminals on azalea and motorized opener; it does not alter routing costs or infer quality from geometry. A floating-point boundary discrepancy at tmc_STEP (~5e-10 boardunits) was excluded using1e-6boardunit diagnostic tolerance. Motorized has two real branch terminals outside their assigned band: R1 on tmc_ENN and R9 on Net-(R9-Pad2). Azalea coverage audit also saved. This motivates checking whether the added unconnected items involve these branches before proposing a branch-aware cost change. These are inputs parsed with corrected images, not a replay of historical PR27 geometry. Standalone separate target avoids concurrent access to the full integration test target. Source/output retained.

### Current-main corridor review preparation

Merged main 7d33cef into the PR27 branch without committing; resolved running-log conflicts by retaining both histories and restored the separately saved terminal diagnostic log tail. All 600 source/data/manifests match the measured current-main snapshot. Clean workspace build and suite running before the merge commit. No terminal-envelope change included.

The terminal-envelope extension was tested separately on seven completed pilot boards: +3U/−3Cu, four U regressions vs two improvements, zero copper gainers, CPU ratio1.0468. It worsened azalea42→44U and kinetoscope49→54U, while motorized61→59U and GB-LIVE32 5→1U improved. Held after negative pilot; no full run or merge of this extension. The capacity diagnostic also refuted a separate explanation for azalea: old/new pad metadata give the same /RESET band, where endpoint span210099.5065 dominates global-rule capacity42192 and trace-pair capacity21336. No capacity patch made. Source and observations retained.

Current-main corridor full complete: 2253 successful KiCad referees, backups on laptop/workbench. Completed control pairs720: −45U/+22Cu; 498 identical SES account −11mask, 222 changed −66mask. OpenHardwareExG accounts +2U/+24Cu (12 shorts, eight clearance, four extra width); original 0U0Cu, no outlier exemption. Azalea41→45U/0Cu both. Keep PR27 draft. Fresh clean updated PR workspace2592passed77ignored0failed. Detailed current-main report replaces stale PR comparison.
### Local NPTH keepout prototype

Created isolated experiment/local-hole-clearance at main028f0a5. Failing-first DSNloaderfixture initially lacked networkscope (no packagekeepoutsinserted); corrected fixture, then observed the actual defect: circularhole keepout clearance2500boardunits versusrequired14750. Two1.625mm-radius keepouts represent a2.75mm drill with0.25mm geometricpadding. New optional hole_diameter_um metadata supplies actualroundNPTHdrill; requiredextra=max(drillradius+localcopperfloor−keepoutradius,0), ceiltointegerboardunits. Appliesonly matchingcomponent/position circularObstacleAreas withdiametercontainedbykeepout. Existingpins andhigherclearancespreserved via sharedclass-floor routine behind separatepin/obstacle guards.

Threefocusedloadertests pass including actualholepaddingregression andoldpin/invalidcoppertests. Added generator annotation forroundNPTHdrills withpadsize<=drill. No pilot/fullmeasurementyet; furtherguardtests andfrozeninputverificationrequired. Thisprototype is separatefrommergedPR29 andPR31. No committingbeforefullworkspace/corpusvalidation.

### Local-hole guards and pilot preparation

Sixfocusedloadertests pass:existingpaddingaccounting,preservationofhigherclearance,unrelated/absentdiameter/misposition/oversizeddrill no-op,invaliddiameter rejection,existingpinmaskgapandinvalidcopper tests. Fixturehelper corrections:read_board returnsBox<Board>; higher-rule assertion must restricttoObstacleAreas,notBoardOutline. No production changes were needed for these test-fixture corrections.

Laptop hit ENOSPC during archive/build. Removed only task-generated disposablecompileroutputs (corridor diagnostic/test targets and completedsharedimage target) and incompletearchive; freed~2GiB. Source/resultsretained; metadata/serverbenchunaffected. Rebuiltfocusedtests successfully, regeneratedcompletearchive and finalpatch. Sharedtarget is nowempty; no fullworkspaceclaim for localholeprototype.

Holemetadata generator2261394 createsseparate751boarddirectory. Pilotsetup freezesmain028f0a5+prototype, validatesonly optionalhole_diameter_um fields changeversusmetadata05 and targetboardsannotated. Fourboards:balena,esp32,Sensors,OpenHardwareExG; main/control/holes withsamecandidatebinaryforcontrol/holes. Serializedafter imagefull389871;192jobs1thread10passes300s; refereeenvandrepair/exportpresent. Noqualityclaimyet.

### PR31 merged; local-hole pilot measured

PR31 merged7d33cef2784b7eac2c6c341d2138b44b1b11a28a at2026-09-10T01:39:37Z, reviewedHEAD7bd10d22cefe035658009a4343fb1164a3fb16a8. Baseline028f0a5 recheckedunchanged beforemerge; source/tests/tooling equality verifiedaftermerge. Freshsuite2587passed77ignored0failed;generatedfullreport17qualitylossesretained. Completedagainstnewpadmain−15U/−46Cu; againstpre29maind84combined−14U/−186Cu. Allordinaryregressionsexplicit;originalJVMrecordingsunchanged.

Localholepilot2320983 completed12KiCadok,allCOMPLETED, base028f0a5 beforeimagefix:esp32 0U1Cu→1U0Cu;balena0U2Cu→0U0Cu;Sensors0U0Cuboth;OpenHardwareExG1U30Cuboth. Thus+1U/−3Cu,supportingphysicalrulecorrectionbutnotaconnectiongainclaim. Samebinarycontrol matchesmainqualityallfour. Backedup locally/workbench.

Fast-forwarded localholeworktree to newlymergedmain7d33cef without losinguncommittedprototype; preserved/reappendedownlogtail. Startedfullworkspace46860 in existingdedicatedlocalhole target (sameworktree,normalnewmainrecompile). Frozennewmainarchive+holeoverlay sentserver; fullquality-epyc-local-hole-full-01 setup launched,751eachmain/control/holes,192jobs1thread10passes300s. Baseline uses previouslyverifiedcombinedbinary equal7d33cef;control/holes same newcandidatebinary withmetadata05/holemetadata01. Inputvalidation andrefereerescore/details included. Nofullholequalityclaimyet.

### Corridor clearance-capacity audit prepared

Current PR27 computes lane capacity using the maximum clearance across every item class, including newlyimportedlocalpadfloors. Prepared standalone read-only audit reusing exact corridor selection and public CLIloadpipeline (projectplusold/newsidecar), reporting globalmatrixcapacity versus selectedsignal trace-class pair capacity on azalea /RESET. Unlike earlier geometric-only audit, this loads the same metadata as routing. Compilation waits for active localholeworkspacesuite to finish because it will reuse thatsame dedicatedtarget; no concurrentbuilds intoone target. No algorithmchangeorcapacityresult yet.

Full localhole driver2334419 nowrouting. Inputvalidation:all751files differonlybyoptionalholediameter;22boards/183recordsannotated. No currentresultclaim. Disklow again duringfullsuite growth; removedonly four already-transferred reproducible gitarchives,keepingpatches,commitsandallmeasurementresults.

### Corridor capacity hypothesis tested on azalea

Public CLIload with identicalprojectfile andoldmaskmetadata versusnewlocalcoppermetadata gives exactlyequal /RESET bands. Sixsignals:globalcapacity42192boardunits,trace-paircapacity21336,terminalspan210099.5065. The projectedterminalspan dominatesacrosswidth; newmetadata doesnotchangeglobalcapacityhere. Thus the proposedpost-pad inflation doesnot explainazalea'sregression. No capacitypatchmade. Instrumentedstandalonecopy only,productionroutingunchanged; logs/source retained.

Currentlocalholeworkspacesuite46860 completed2591passed77ignored0failed includingdoctests. Afterthiscompletion anddiagnosticcompletion, cleanedonlyitsdisposabletarget toavoidENOSPC. Createdexperiment/corridor-current-main at7d33cef andappliedonlythefiveoriginalPR27routerfiles (aa92bf1→98f827a). This excludes rejectedterminal-envelopeextension. Sincecorrectimage/padinput fixes have landed, remeasuretheexistingstructuralalgorithm againstcurrentmain before deciding whetheritsold ordinaryregressions persist. No currentmaincorridor resultsyet.

### Hole obstacle-index review and corridor suite completed

Reviewed change_clearance_class_index: clearsderiveddata and reinsertscompensatedobstacleentries; raw-shape queries readthematrix withoutcompensation. Addedactualoverlapping_items_with_clearance assertions to the originalhole regression:probeoutsideoldclearance mustbe returned afterraisingthehole rule,onbothcopperlayers. Onlytestchanged; frozenproductionunchanged. Freshfullworkspace3272running. Firstholefullsuite2591passed remains recorded; newassertionnotclaimedpassingyet. Addedmechanism/pilot/inputvalidation report with pendingfullresults.

Corridor-current-main workspace3577 exits0,2592passed77ignored0failed includingdoctests. Cleareditscompletedsharedtargetbefore switchingbacktolocalholeworktree forqueryvalidation; no concurrentbuildsinsametarget. Fullhole2334419live,corridor3610920queued.

### Local hole full run completed; corridor transfer repaired

Full local-hole run scored all 2,253 KiCad cases and is backed up on laptop/workbench. Completed same-binary pairs: 722 boards, 0 unrouted / −9 copper; ESP32 −1U/−2Cu, balena −3Cu, bikedar −4Cu, EncoderBoard +1U. No completed copper gainers. Raw PC −433U/−8Cu/+639mask, CPU .9481; local quality unchanged, CPU .9975. All +612 completed mask differences occur on 712 byte-identical SES outputs; ten changed outputs have zero mask delta. Main/control completed outputs all identical. See local-hole report for full main comparisons. Investigate EncoderBoard before acceptance; do not claim raw timeout improvements.

Current-main corridor upload omitted untracked corridor.rs, so the serialized driver stopped at compilation before routing any boards. Copied the missing file and verified all 600 crate/manifests/data hashes against the locally tested worktree; relaunched as driver 57343. No algorithm change. Local-hole query regression assertion initially failed Rust borrowing rules; fixed by collecting obstacle identifiers/classes/layers before mutable queries. Fresh workspace verification running.

Local-hole final query workspace verification: 2,591 passed, 77 ignored, zero failed, including doctests. Encoder original audit: /LED14 is 28.86mm/two vias vs control 32.10mm/four vias, both >10mm from annotated mounting holes. Candidate leaves net wholly unrouted; indirect routing interaction, no outlier exemption. Generated benchmark gate fails seven quality losses and is retained.

## EncoderBoard pass-budget diagnostic

`quality-epyc-local-hole-encoder-20pass-01` reroutes only EncoderBoard with the same frozen control/holes executables and metadata, 20 passes, a 600-second diagnostic cap, and two single-thread jobs. Both complete, with successful KiCad referees, **0 unrouted and 0 copper violations**. Control/candidate CPU is 108.76/100.71 seconds, vias34/39 and wire1046.24/1058.25mm. This shows the missing connection can be recovered with further routing; it does not replace the 10-pass acceptance comparison or justify a general runtime claim or global budget increase. The 10-pass +1U regression remains reported.

### Shove revalidation prototype

Root cause in the captured replay: protected-prefix probes are clear through shove steps0–8; step9 moves AGND back across previously checked segments1–3. Those segments remain obstructed through step13. No speculative fix was applied before this observation. A final same-rules check of the post-shove multi-segment path, with all shove/spring recursion disabled, rejects this path before raw insertion and makes the failing test pass (returns original corner, zero new short). The check extends existing guards; no existing shove check is replaced. It is experimental under COPPERROUTE_RECHECK_SHOVED_PATH.

The cropped replay still fails after reducing board items to51 (outside component pins explicitly unassigned/unfixed for diagnostic removal). The committed-style regression currently uses the full captured DSN for faithful context and reexecutes only itself with the experimental switch enabled. No JVM recordings changed. Temporary logs/copper-dsn production dependency/capture code are excluded from new worktree experiment/shove-revalidation at currentmain54d4e80.

Full EPYC driver1986066 starts quality-epyc-shove-revalidation-full-01: main, same-binary control, guard, corridor, corridor+guard;751KiCadboards each,10passes300seconds1thread192jobs. All candidates use merged local-hole metadata01 and the same project floors; main executable is the production snapshot verified for PR32.600 source/data hashes verified for new frozen build. Builds/tests/results pending. Clean local workspace suite runs in the new worktree, shared target cleaned after prior diagnostic finished. No quality acceptance or speed claim yet.

Shove-revalidation first clean workspace passed2597top-level tests plus one child guard replay,77ignored0failed. Raw log sums2598because it includes that child result twice across parent/child harnesses. Added complementary clear-prefix test: same fixture, three-point unobstructed path still reaches endpoint with guard enabled. Both scoped subprocess tests pass. Final workspace rerun includes added positive case; production source remains identical to frozen benchmark. Source review confirms recursion0 exits before substitute trace generation and before via movement; no item IDs are consumed by that branch. Full result postprocess driver2811620 waits on1986066, verifies751successfulKiCad rows per candidate, checks completed SES identities, and generates main comparison for guard/corridor/combined.

Final shove-revalidation workspace suite completed: 2,598 top-level passed plus two isolated child checks, 77 ignored, zero failures (raw harness sum 2,600). Both invalid-path rejection and clear-prefix completion are covered. Full corpus driver1986066 and postprocess2811620 remain live; workbench backup522266 verified live. No corpus acceptance claim before all five candidates are scored.

During full-run validation, OpenHardwareExG Shield main/control/guard completed with successful KiCad scores of 1U/30 copper (all track-width). Corridor reproduces 3U/54 copper: 12 shorts, 8 clearance, 34 track-width. Combined result remains pending at observation; these selected results are diagnostic, not corpus acceptance. Prepared six-pair summary script with 751-board and all-referees-ok assertions, separate PCBench/local and completed-pair views, CPU/RSS and per-board quality deltas. Preserved it alongside local test evidence. Driver remains live; its failed-referee rescoring stage precedes exports.

Full-board targeted result from the ongoing full run: all five OpenHardwareExG Shield cases completed with successful KiCad referees. Main/control/guard: 1U, 30 copper violations (track width). Corridor: 3U, 54 copper (12 shorts, 8 clearance, 34 width). Corridor+guard: 3U, 34 copper (all width). Thus the final path guard removes all 20 new shorts/clearance violations without changing the corridor result's unrouted count. Relative to main, +2U/+4 width violations remain counted on this ordinary board. Full corpus acceptance remains pending. Raw SES/metrics/KiCad reports for all five cases are preserved locally in shove-revalidation-shield-diagnostic.tar.gz and mirrored through the server reports backup.

Shield residual width audit: all 30 main and all 34 corridor/combined track-width errors report actual0.2488mm against project minimum0.2540mm. Corridor and combined have identical width counts by net: AGND16, AVDD7, 3.3V_ISO3, ADS129x_GPIO4_ISO2, C19 signal2, C42 signal2, AVDD1 one, 3.3VADC one. This is a width-floor issue distinct from the proven shove-overlap defect; the +4 remains counted, and no width-policy patch was made during the frozen run.

## Full KiCad result against main 54d4e80

quality-epyc-shove-revalidation-full-01 completed all 3,755 cases: five variants × 751 boards, 10 passes, 300-second cap, 192 single-thread workers. Every referee status is ok after rescoring failed referees on unchanged SES outputs. Results are preserved on the laptop and mirrored to workbench.

| Comparison | Group | Raw delta U | U better/worse | Raw delta copper | Copper gainers | CPU ratio | Completed pairs | Paired U / copper |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| main-vs-guard | pcbench | -1 | 6/2 | +1 | 1 | 1.0013 | 709 | +6 / +0 |
| main-vs-guard | local | +0 | 0/0 | +0 | 0 | 1.0062 | 10 | +0 / +0 |
| main-vs-corridor | pcbench | -47 | 39/18 | +17 | 8 | 1.0006 | 709 | -47 / +20 |
| main-vs-corridor | local | +4 | 0/2 | +1 | 1 | 1.0142 | 10 | +3 / +1 |
| main-vs-corridor-guard | pcbench | -282 | 45/16 | -1 | 8 | 0.9432 | 709 | -39 / -1 |
| main-vs-corridor-guard | local | +4 | 0/2 | +1 | 1 | 1.0194 | 10 | +3 / +1 |
| corridor-vs-corridor-guard | pcbench | -235 | 14/4 | -18 | 3 | 0.9426 | 713 | +8 / -21 |
| corridor-vs-corridor-guard | local | +0 | 0/0 | +0 | 0 | 1.0051 | 10 | +0 / +0 |

Combined versus main on 719 completed pairs: **−36 unrouted / 0 net copper**, with U improving on 29 boards and worsening on 13; copper improves on seven and worsens on seven. PCBench contributes −39U/−1 copper; local KiCad fixtures +3U/+1 copper. No board is excluded as an outlier. Main/control completed SES files are all byte-identical and have 0U/0 copper delta.

The guard versus corridor removes 21 copper violations on completed pairs but adds eight unrouted connections: Shield −20 copper, GB-CART256K-A −1U/−1 copper, boatcontrol_NonLatchingNO30A +6U and uSKY +3U. The latter two regressions need investigation. Guard alone gives +6U/0 copper on completed pairs, so there is no demonstrated standalone aggregate gain on main.

For combined versus main, 492 completed SES outputs are identical (0U/0 copper/−7 mask), while 227 differ (−36U/0 copper/−91 mask). Raw mask counts are retained; identical-output differences cannot be attributed to the routing algorithm. Raw timeout totals and single-run CPU ratios are observations, not causal speed claims.

PCBench main/combined completed counts are 709/717, fully connected 589/599, median RSS12.1/12.1MiB, max372.4/375.4MiB. Local completed10/10, fully connected8/7, medianRSS12.0/13.5MiB, max75.8/105.1MiB. The completed-pair view excludes either deadline/non-COMPLETED outcome.

The generated benchmark gate fails: guard six quality losses, corridor112, combined112. The full generated report is retained in routing-quality-artifacts/shove-revalidation/pr-summary.md; this is not a clean gate. Candidate remains uncommitted pending regression review.

Regressor investigation: original boatcontrol is fully connected with zero copper DRC, 0 vias and 5675.348mm routing. Current control21U/0Cu/12vias/2382.4893mm; guard27U/0Cu/21vias/2768.5775mm. Original uSKY is fully connected with zero copper DRC, 23vias and174.589mm; it has an invalid-outline warning among original non-routing findings. Control21U/0Cu/6vias/74.8513mm; guard24U/0Cu/3vias/70.2005mm. Original-versus-control-versus-guard routing overviews inspected (simplified pad outlines, zones omitted). Boatcontrol shows orderly mostly single-layer routing; no outlier exemption established.

Temporary logging lives only in separate worktree experiment/shove-revalidation-audit. EPYC diagnostic driver980172 built successfully (1m10s) and runs audit-on/audit-off on both boards,10passes300s,4jobs. Early uSKY rejection reports same-net trace (net10) as obstacle, expired=false; this motivates capturing and checking actual clearance before relaxing anything. Same-net reported obstacle alone is not proof of a clear path. First boatcontrol rejection is net27 against net1, layer2,width20000. Diagnostic still live; output identity verification pending.

First regressor observer run completed: all four selected cases COMPLETED and KiCad ok. Audit-on/off SES hashes match each other and the frozen full guard output exactly for both boards. Logs and sessions archived locally; identity/count artifacts retained. New capture-only driver984845 builds the diagnostic source with first-rejection DSN/shape capture and a direct check_trace_shape probe on a deep copy. No production change; capture observer identity verification pending.

Capture probe evidence: first rejected segment on both uSKY and boatcontrol also fails Board::check_trace_shape on a deep copy (direct_clear=false). Therefore the same-net reported obstacle is not sufficient to justify relaxing the guard; the proposed same-net-only explanation is unproven. uSKY shape is octagon[213766,-223494,218343,-221970,436182,441391,-9282,-4073], layer1,class1,net10,width762. Boatcontrol shape is octagon[1550000,-1016922,1636983,-929939,2491655,2642189,591777,648345],layer2,class3,net27,width20000. First-rejection DSNs captured. Diagnostic driver984845 remains live; its source build passed1m11s. No algorithm relaxation made.

Capture replay completed successfully; captures import with no warnings. uSKY route net10=GND: same-net trace185 is not an obstacle, but trace184 on net11 is an obstacle. Boatcontrol route net27=Net-(J102-Pad10): trace914 on net1 is an obstacle; same-net trace915 and pin87 are not. Both direct shape checks reject. This refutes treating the reported same-net obstacle as permission to bypass clearance; no relaxation implemented. Capture observer-on/off/frozen guard SES hashes are identical for both boards; all four KiCad scores are successful and completed.

Combined copper-gainer audit (all retained): Shield +4 track-width; VC4000 +2 shorts against B.Cu text; rp2040-dmxsun baseboard2slots +3 starved thermals; Patternflow +1 starved thermal; ESP07 +1 clearance; pmw3360_jst +1 copper-edge clearance; motorizedopener +1 hole clearance. VC4000 original has0U/0routingDRC; KiCad names text VC4000 MultiROM v0.4 / Keller/Maibaum in every reported short. DSN has no text string, keepout or wire; all13polygon declarations are footprint outlines. This is a confirmed copper-text input omission, analogous to AVR-fuser, not an excuse to drop its +2Cu/−2U contribution.

## Coupled guidance safety validation

Prepared guidance so COPPERROUTE_CORRIDOR_GUIDANCE automatically enables the final path guard. The standalone experimental recheck switch remains available; the guidance switch stays opt-in. This prevents enabling the structural routing experiment without its required safety check. New regression corridor_guidance_also_revalidates_paths failed first: expected original corner but reached804926/−701946 with guidance alone. After sharing the guidance-enabled predicate with the final guard, all three guard tests pass. No JVM recordings changed.

Final workspace suite35303 running against this source. EPYC driver991239 starts quality-epyc-shove-coupled-full-01, main/control/coupled,751KiCadboards each,10passes300s1thread192jobs.602source/data/test/fixture hashes match local before launch. Uses the same merged local-hole metadata01 and project floors. Failed referees rescore without rerouting before exports. New-source acceptance and final workspace completion pending; prior five-way result remains an experiment result, not a claim that this new source has completed qualification.

Final coupled-source workspace verification completed (session35303 terminal0): 2,599 top-level tests plus three isolated guard checks passed,77ignored0failed. Raw harness sum2602 includes children. Evidence: coupled-workspace-summary.txt and preserved full local log. Corpus991239/postprocess1312504 remain live; no final quality acceptance claim.

PR27 final-source preparation: merged main54d4e80 into existing head43d9a18 with --no-commit. Only running-log conflict occurred; both histories retained. Applied the coupled guard and all its regression/report artifacts. All602source/data/test/fixture hashes match the active frozen EPYC candidate. Fresh workspace12447 runs on the actual PR branch after cleaning the completed prior shared build target. No merge commit or publication until this suite and final corpus comparison are complete.

Exact geometry audit strengthens uSKY rejection: captured proposed GND path and existing net11 share 4 complete centerline segments (undirected endpoints matched exactly). Both halfwidths762. This is true intermediate copper overlap, not just an overly wide clearance bound or same-net restriction. Keep guard; final main/control0Cu does not imply every intermediate insertion was safe. Artifact usky-shared-segments.json.

Fresh actual PR27-branch workspace suite12447 completed terminal0:2599top-level passed plus3childchecks,77ignored0failed. All602source/data files reverified against frozen coupled source after the suite. Full log preserved locally; pr27-workspace-summary.txt retained. Merge remains uncommitted pending corpus991239 and postprocess1312504, both verified live after final board submissions.

Final coupled full run completed and backed up:2253KiCadok. Main719completedpairs−36U/0copper;control720pairs same.726previous-combined/current completed SES outputs identical. Fresh actualPR27workspace2599top-level+3childchecks passed,77ignored0failed;602source/datahashesmatch. Report rewritten around final automatic guard; generated112-loss gate included. Raw main PC−190U/−1copper/−107mask,CPU.9431;local+4U/+1copper/0mask,CPU1.0108. CompletedPC−39U/−1copper,local+3U/+1copper. Ordinary regressions and VC4000 copper-text input omission remain counted. Preparing authorized opt-in PR update, not default activation.
