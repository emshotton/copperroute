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

Active validation: workbench via-progress, plus temporary-server batch
`quality-epyc-full-01` with main, census, via-progress, via-projection,
nominal+smoothing+census and smoothing-noop. The replaced workbench waiters
are stopped; results and artifacts are continuously copied back to workbench.
The cache+two-via-guards composition has local tests only and is not yet in
a full-corpus run.

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
