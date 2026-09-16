# After-optimization repair: experiment history and results

This document records the recent routing-algorithm investigation leading to the opt-in after-optimization repair. It includes unsuccessful approaches so they are not mistaken for untested ideas. Earlier project-wide DRC, import, corridor and routing investigations remain in [the running log](routing-quality-log.md); the quantitative comparisons here concern the algorithm study based on `66612acf2efe7e2131dcbb0bfc86387d24e3f111` in September 2026.

## Decision

Retain the simplest conflict-based repair **after the optimizer** as a reviewable, opt-in candidate. The earlier combined-guidance, pass-3 group-repair, reservation and neighbor-preservation variants are not included in this branch. This is not a claim that all conflict-based routing is beneficial, or that this branch is ready to merge.

In the full terminal experiment, 723 pairs completed before their deadlines: seven fewer missing connections across five boards, no connectivity regressions, and unchanged copper DRC. Three more boards are fully connected in the headline comparison. CPU is effectively unchanged. The benefit is small and must be weighed against the implementation and maintenance cost.

## Measurement and interpretation

- KiCad 10.0.4 is the referee. No Java DRC results are included. The full imported eligible corpus is 751 boards: 740 PCBench and eleven local KiCad fixtures.
- Normally ten passes, 300 seconds, one router thread, twelve workbench jobs. The original 32-board demand/target screen used four jobs; the initial 16-board group screen used 600 seconds. These exceptions must not be compared as identical experiments.
- Diagnostic subsets were deliberately selected to include difficult boards, gains and regressions. They are not representative estimates of the full corpus.
- Positive quality deltas mean worse. Copper and mask findings are reported separately. Equal totals can hide individual regressions; gaining-board counts are retained below.
- A board reaching the time limit can return different intermediate geometry under CPU contention. Report both the raw totals and pairs where both candidates completed normally. CPU ratios use CPU seconds, never parallel wall time, and are not speed claims from a single repetition.
- KiCad sometimes caps reports near 199 findings per type. Different reported counts with byte-identical SES output cannot establish an algorithmic improvement or regression. Keep those counts visible and audit output identity separately.
- Later runs reused the verified 300-second main baseline from `target-search-full-01`; baseline binary, manifest and all input DSN hashes were checked. Separate-run CPU measurements have that limitation. This is the evaluated base commit, not a claim about an arbitrary future main.

## What each approach changed

1. **Forecast congestion:** estimate other nets' likely paths using geometric spanning trees and alternative L-shaped routes. Penalize predicted busy regions during maze expansion. The coarse forecast omits important obstacle capacity and final topology; normally completed boards got worse.
2. **Improved target search:** keep separate destination boxes so the search does not mistake empty space between distant targets for a nearby destination. The promising small-screen headline did not survive full-corpus evaluation: +280 missing connections and +14 copper findings overall; +17 missing connections on normally completed pairs.
3. **Combined recovery v1:** enable demand/target guidance after whole-board progress stalls. It could replace a normal pass that would have done better. Feather and BMS regressed; headline gains came from deadline boards that never activated recovery.
4. **Combined recovery v2:** run the ordinary pass first and compare a recovery alternative from a saved board. This removed the v1 regressions, but all 29 normally completed SES outputs were identical to baseline. No demonstrated final improvement.
5. **Individual connection stalls:** track unchanged fixed-terminal components rather than whole-board stagnation, with a cooldown between attempts. This activated recovery on previously missed cases, but normally completed pairs still added three missing connections and the number of fully connected boards fell.
6. **Group rebuild control:** remove movable routing for an unfinished signal net and up to two nearby nets, then independently reroute the group. Accept only fully connected groups and a strict whole-board internal connectivity improvement without extra internal DRC. This isolates the effect of reconsidering a group; it does not solve incompatible independent plans.
7. **Topological alternatives:** add temporary cuts from an obstacle towards opposite board edges, generating routes on different sides or layers; evaluate combinations. The bounded prototype did not show a final connectivity gain. HBR worsened despite a connected, DRC-clean original. This does not disprove general topological routing or MCTS.
8. **Early conflict-based search:** independently route group nets; at a copper conflict, branch on which net must avoid a small forbidden region. Reroute that owner and retain accumulated constraints. Only 52 of 405 conflicting groups were solved in the full early run; 101 groups were retained including initially compatible plans. On normally completed full-corpus pairs, connectivity was unchanged, with mask regressions. Early local improvements did not guarantee good final boards.
9. **Whole-route reservations:** before falling back to the small-region replan, try routing the selected net around the other plans' complete copper geometry. This improves some individual boards, but the early variant worsens normally completed connectivity overall. SRAMBO completes but still acquires mask findings.
10. **Late repair before optimization:** move the same group repair after ordinary routing's best-board restore and tail cleanup. Both point and reservation variants still alter optimizer input. BMS and Sizif regressions are ordinary signal incompletes visible internally, not evidence of zone-refill mismatch on those particular boards.
11. **Preserve complete neighbors:** initialize already connected neighboring nets with their existing movable copper rather than rebuilding them immediately. Reroute them only when a conflict requires it. At pass 3 this still worsens completeness. Before optimization it reduces mask findings substantially, mainly on NRC2016, but trades two improvements against BMS becoming incomplete.
12. **After-optimization repair:** run the original point-conflict search only after the ordinary pipeline and optimizer have finished. The reservation version was also screened at this stage and produced the same single normally completed gain; it adds complexity without demonstrated extra benefit. The full evaluation therefore uses the simpler point-conflict version.

These are bounded group planners around the existing detailed maze engine, not wholesale replacements of geometric routing. Eight search nodes, at most three groups of two or three 2–16-pin signal nets, and a 30-second per-group limit constrain the experiment. The global deadline/cancellation still applies. Finite geometric exclusion boxes do not provide the completeness or optimality guarantees of discrete multi-agent conflict-based search.

## All recorded comparisons

Each row is relative to that run's matched baseline, not the previous table row. CPU and peak RSS cover all paired boards. “Normal Δ” is U/copper/mask on pairs where both runs finished before the deadline. Reused arms in later reports are labeled; they are not additional experiments.

| Screen / variant | Boards | U Δ (better/worse boards) | Copper Δ / gainers | Mask Δ / gainers | CPU ratio | Peak RSS MiB | Fully connected baseline→variant | Normal pairs: Δ U/Cu/mask |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| [32-board demand/target screen](routing-quality-artifacts/after-optimization-repair/congestion-subset/report.md) / demand | 32 | -14 (8/12) | +8 / 4 | +23 / 2 | 0.980× | 144.9 | 10→10 | 29: +18/+7/+23 |
| [32-board demand/target screen](routing-quality-artifacts/after-optimization-repair/congestion-subset/report.md) / targets | 32 | -50 (5/6) | +6 / 2 | +69 / 3 | 0.795× | 117.7 | 10→10 | 29: +6/+2/+69 |
| [751-board target search](routing-quality-artifacts/after-optimization-repair/target-full/report.md) / targets | 751 | +280 (36/52) | +14 / 28 | -28 / 33 | 1.065× | 254.6 | 602→601 | 714: +17/-14/-4 |
| [32-board combined recovery v1](routing-quality-artifacts/after-optimization-repair/adaptive-v1/report.md) / adaptive | 32 | -37 (3/2) | -3 / 0 | +5 / 1 | 0.973× | 179.3 | 10→9 | 29: +8/+0/+5 |
| [32-board combined recovery v2](routing-quality-artifacts/after-optimization-repair/adaptive-v2/report.md) / alternatives | 32 | -49 (2/1) | +3 / 1 | +0 / 0 | 0.994× | 180.1 | 10→10 | 29: +0/+0/+0 |
| [32-board individual stall recovery](routing-quality-artifacts/after-optimization-repair/connection-recovery/report.md) / alternatives | 32 | +3 (0/2) | -2 / 0 | +0 / 0 | 1.023× | 179.5 | 10→10 | 29: +0/+0/+0 |
| [32-board individual stall recovery](routing-quality-artifacts/after-optimization-repair/connection-recovery/report.md) / connections | 32 | -22 (5/5) | -1 / 0 | +1 / 1 | 1.045× | 180.7 | 10→9 | 29: +3/+0/+1 |
| [16-board group planning; 600 seconds](routing-quality-artifacts/after-optimization-repair/group-screen/report.md) / rebuild | 16 | -1 (1/1) | +3 / 1 | +0 / 0 | 1.075× | 173.2 | 4→4 | 14: +0/+0/+0 |
| [16-board group planning; 600 seconds](routing-quality-artifacts/after-optimization-repair/group-screen/report.md) / topology | 16 | -9 (2/1) | +3 / 1 | +0 / 0 | 1.035× | 173.4 | 4→4 | 14: +2/+0/+0 |
| [16-board group planning; 600 seconds](routing-quality-artifacts/after-optimization-repair/group-screen/report.md) / conflicts | 16 | -14 (5/0) | +3 / 1 | +29 / 1 | 0.886× | 176.4 | 4→5 | 14: -3/+0/+29 |
| [751-board early conflict search](routing-quality-artifacts/after-optimization-repair/conflict-full/report.md) / conflicts | 751 | -55 (16/18) | +0 / 7 | +68 / 20 | 1.033× | 295.4 | 602→601 | 723: +0/-5/+68 |
| [42-board reservations/timing screen](routing-quality-artifacts/after-optimization-repair/reservations-screen/report.md) / control | 42 | -15 (2/0) | +2 / 1 | -1 / 0 | 0.987× | 179.9 | 19→19 | 39: +0/+0/+0 |
| [42-board reservations/timing screen](routing-quality-artifacts/after-optimization-repair/reservations-screen/report.md) / reserved | 42 | +7 (10/12) | +10 / 7 | +6 / 6 | 1.000× | 184.5 | 19→16 | 39: +5/-6/+7 |
| [42-board reservations/timing screen](routing-quality-artifacts/after-optimization-repair/reservations-screen/report.md) / late | 42 | +13 (2/3) | -2 / 1 | +1 / 1 | 0.997× | 180.2 | 19→19 | 39: +4/-6/+1 |
| [42-board reservations/timing screen](routing-quality-artifacts/after-optimization-repair/reservations-screen/report.md) / reservedlate | 42 | -29 (3/3) | -3 / 1 | -35 / 1 | 0.967× | 178.3 | 19→18 | 39: +6/-6/-34 |
| [42-board terminal repair screen](routing-quality-artifacts/after-optimization-repair/terminal-screen/report.md) / control | 42 | -5 (3/0) | +2 / 1 | -1 / 0 | 0.988× | 179.2 | 19→19 | 39: +0/+0/+0 |
| [42-board terminal repair screen](routing-quality-artifacts/after-optimization-repair/terminal-screen/report.md) / final | 42 | +6 (2/1) | +4 / 1 | -1 / 0 | 0.992× | 182.1 | 19→20 | 39: -1/+0/+0 |
| [42-board terminal repair screen](routing-quality-artifacts/after-optimization-repair/terminal-screen/report.md) / reservedfinal | 42 | -34 (3/0) | +3 / 1 | -1 / 0 | 0.975× | 179.6 | 19→20 | 39: -1/+0/+0 |
| [42-board preserved-neighbor screen](routing-quality-artifacts/after-optimization-repair/preserve-screen/report.md) / preserve | 42 | +11 (8/9) | +4 / 4 | -2 / 6 | 0.982× | 177.5 | 19→18 | 39: +7/+0/-1 |
| [42-board preserved-neighbor screen](routing-quality-artifacts/after-optimization-repair/preserve-screen/report.md) / preservelate | 42 | -31 (4/1) | +1 / 1 | -56 / 1 | 0.956× | 179.4 | 19→19 | 39: +0/-2/-55 |
| [751-board terminal repair](routing-quality-artifacts/after-optimization-repair/terminal-full/report.md) / final | 751 | -78 (12/6) | +2 / 1 | +33 / 15 | 1.000× | 295.4 | 602→605 | 723: -7/+0/+31 |

## Split by board origin

Both groups use KiCad. The Java-only fixture group is excluded. Per-board tables and complete raw summaries are linked above.

| Screen / variant | Origin | Boards | U Δ (better/worse) | Copper Δ / gainers | Mask Δ / gainers | CPU ratio |
|---|---|---:|---:|---:|---:|---:|
| congestion-subset / demand | pcbench | 28 | -15 (8/11) | +8 / 4 | +23 / 2 | 1.001× |
| congestion-subset / demand | local | 4 | +1 (0/1) | +0 / 0 | +0 / 0 | 0.795× |
| congestion-subset / targets | pcbench | 28 | -50 (5/6) | +6 / 2 | +69 / 3 | 0.827× |
| congestion-subset / targets | local | 4 | +0 (0/0) | +0 / 0 | +0 / 0 | 0.508× |
| target-full / targets | pcbench | 740 | +278 (35/51) | +14 / 28 | -28 / 33 | 1.069× |
| target-full / targets | local | 11 | +2 (1/1) | +0 / 0 | +0 / 0 | 0.928× |
| adaptive-v1 / adaptive | pcbench | 28 | -37 (3/2) | -3 / 0 | +5 / 1 | 0.986× |
| adaptive-v1 / adaptive | local | 4 | +0 (0/0) | +0 / 0 | +0 / 0 | 0.847× |
| adaptive-v2 / alternatives | pcbench | 28 | -49 (2/1) | +3 / 1 | +0 / 0 | 1.020× |
| adaptive-v2 / alternatives | local | 4 | +0 (0/0) | +0 / 0 | +0 / 0 | 0.755× |
| connection-recovery / alternatives | pcbench | 28 | +3 (0/2) | -2 / 0 | +0 / 0 | 1.026× |
| connection-recovery / alternatives | local | 4 | +0 (0/0) | +0 / 0 | +0 / 0 | 0.991× |
| connection-recovery / connections | pcbench | 28 | -22 (5/5) | -1 / 0 | +1 / 1 | 1.057× |
| connection-recovery / connections | local | 4 | +0 (0/0) | +0 / 0 | +0 / 0 | 0.928× |
| group-screen / rebuild | pcbench | 12 | -1 (1/1) | +3 / 1 | +0 / 0 | 1.077× |
| group-screen / rebuild | local | 4 | +0 (0/0) | +0 / 0 | +0 / 0 | 1.062× |
| group-screen / topology | pcbench | 12 | -9 (2/1) | +3 / 1 | +0 / 0 | 1.025× |
| group-screen / topology | local | 4 | +0 (0/0) | +0 / 0 | +0 / 0 | 1.122× |
| group-screen / conflicts | pcbench | 12 | -14 (5/0) | +3 / 1 | +29 / 1 | 0.894× |
| group-screen / conflicts | local | 4 | +0 (0/0) | +0 / 0 | +0 / 0 | 0.816× |
| conflict-full / conflicts | pcbench | 740 | -54 (15/18) | +0 / 7 | +68 / 20 | 1.035× |
| conflict-full / conflicts | local | 11 | -1 (1/0) | +0 / 0 | +0 / 0 | 0.940× |
| reservations-screen / control | pcbench | 38 | -15 (2/0) | +2 / 1 | -1 / 0 | 1.003× |
| reservations-screen / control | local | 4 | +0 (0/0) | +0 / 0 | +0 / 0 | 0.760× |
| reservations-screen / reserved | pcbench | 38 | +7 (10/12) | +10 / 7 | +6 / 6 | 1.001× |
| reservations-screen / reserved | local | 4 | +0 (0/0) | +0 / 0 | +0 / 0 | 0.990× |
| reservations-screen / late | pcbench | 38 | +13 (2/3) | -2 / 1 | +1 / 1 | 1.005× |
| reservations-screen / late | local | 4 | +0 (0/0) | +0 / 0 | +0 / 0 | 0.880× |
| reservations-screen / reservedlate | pcbench | 38 | -29 (3/3) | -3 / 1 | -35 / 1 | 0.987× |
| reservations-screen / reservedlate | local | 4 | +0 (0/0) | +0 / 0 | +0 / 0 | 0.695× |
| terminal-screen / control | pcbench | 38 | -5 (3/0) | +2 / 1 | -1 / 0 | 1.002× |
| terminal-screen / control | local | 4 | +0 (0/0) | +0 / 0 | +0 / 0 | 0.782× |
| terminal-screen / final | pcbench | 38 | +6 (2/1) | +4 / 1 | -1 / 0 | 1.003× |
| terminal-screen / final | local | 4 | +0 (0/0) | +0 / 0 | +0 / 0 | 0.836× |
| terminal-screen / reservedfinal | pcbench | 38 | -34 (3/0) | +3 / 1 | -1 / 0 | 0.997× |
| terminal-screen / reservedfinal | local | 4 | +0 (0/0) | +0 / 0 | +0 / 0 | 0.665× |
| preserve-screen / preserve | pcbench | 38 | +11 (8/9) | +4 / 4 | -2 / 6 | 0.991× |
| preserve-screen / preserve | local | 4 | +0 (0/0) | +0 / 0 | +0 / 0 | 0.861× |
| preserve-screen / preservelate | pcbench | 38 | -31 (4/1) | +1 / 1 | -56 / 1 | 0.978× |
| preserve-screen / preservelate | local | 4 | +0 (0/0) | +0 / 0 | +0 / 0 | 0.639× |
| terminal-full / final | pcbench | 740 | -77 (11/6) | +2 / 1 | +33 / 15 | 1.003× |
| terminal-full / final | local | 11 | -1 (1/0) | +0 / 0 | +0 / 0 | 0.875× |

## Terminal full-run evidence

Run `conflict-terminal-full-01` has 751 valid KiCad comparisons after retrying one transient Curryboard Gtk/SES-import failure. The retry reused its routed output. Main versus candidate:

- Unrouted: 4,584→4,506; 12 improved boards, six regressed.
- Copper findings: 869→871. The sole gaining board is decelerator4030; both sides hit their deadline.
- Reported mask findings: 13,903→13,936; nine improving and fifteen gaining boards. Every normally completed mask-gaining board has a capped report and byte-identical SES output to baseline. On 724 uncapped pairs, mask findings fall by seven, with no gainers; this alone does not establish physical geometry improvement.
- Fully connected: 602→605. Deadline outcomes: 27 on each side.
- CPU: 32,672.23→32,658.04 seconds (0.9996×). Highest peak RSS: 295.2→295.4 MiB.

The more defensible connectivity result is **seven fewer missing connections on 723 normally completed pairs**, with no connectivity or copper regressions. The five improved boards are:

| Board | Missing connections before→after |
|---|---:|
| DoroidOscillo Android Oscilloscope | 5→4 |
| Solare BQ24210 | 1→0 |
| GB-CART256K-A | 14→12 |
| Starfish | 1→0 |
| uSKY | 24→22 |

Do not attribute the full headline −78 connections to this mechanism: deadline variation accounts for most of it. [Mask output-identity audit](routing-quality-artifacts/after-optimization-repair/terminal-full/mask-gainers-audit.json) retains each apparent gaining board.

## What original boards and failures taught us

- **SRAMBO:** designer routing has 39 vias, versus baseline92 and early conflict repair58; the latter completes the board but raises mask findings136→165. Fewer vias and completion do not excuse a mask regression. Ordered original bundles suggest topology matters, but the tested local group planner is not a bus planner.
- **Relay controller FRM16:** `Made In Chicago` and other artwork sit on B.Mask. Original and baseline avoid those openings; changed rear copper can cross them. This is a previously documented valid unusual feature, not grounds to remove the board from the headline benchmark. The early conflict candidate has25 mask findings versus0, and20vias versus5baseline/6original.
- **Analog switch matrix and rp2040-dmxsun:** some regressions are KiCad ground-pad/zone disconnections after refill; their originals are connected and have zero routing DRC. These are not established outliers.
- **BMS and Sizif:** the before-optimizer variants create ordinary signal incompletes, also visible to our checker. Moving repair after optimization avoids interfering with the successful normal completion on these boards. Keep this diagnosis separate from zone-fill problems.
- **Input fidelity:** DSN does not carry all original mask apertures/artwork. The internal full DRC call cannot test geometry absent from its board model. Its acceptance guard is useful but does not guarantee KiCad parity.

## What this branch contains and how to reproduce

Only the terminal point-conflict implementation is retained. Enable it with:

```sh
COPPERROUTE_AFTER_OPTIMIZATION_REPAIR=1 copperroute route in.dsn -o out.ses
```

It is disabled by default. No early-pass, topology, reservation, or neighbor-preservation modes remain. Scratch-board repairs protect outside copper, respect the net filter, skip plane nets, and replace the final board only for a strict internal missing-connection reduction with no internal routing-DRC increase. No temporary obstacles are exported. KiCad remains authoritative.

The historical full run used the equivalent research flags `COPPERROUTE_GROUP_PLANNING=conflicts` and `COPPERROUTE_GROUP_TIMING=final` on the frozen v3 binary. This cleaned branch removes other modes and diagnostic printing; the full-run numbers above are **from that evaluated prototype**, not an assertion that the cleaned source was independently rerun on all751boards. Cleanup validation reruns workspace tests and ten boards: all five normal full-run gains plus BMS, Sizif, Mouse, SRAMBO and FRM16. The cleanup results are recorded below.

Heavy data remains on workbench under `/home/em/copperroute-congestion-main/benchmark/results/`. Run IDs are retained in each report; launch/provenance directories include `/home/em/copperroute-terminal-full-artifacts` and `/home/em/copperroute-after-repair-validation`. Small reports, summaries and paired CSVs accompany this document. Workspace tests passed for the evaluated prototypes; no JVM recordings were silently replaced.

## Cleanup validation completed

`cargo test --workspace`: **2,808 passed, zero failed, 77 ignored**. Release build passed. Run `after-repair-cleanup-01`: all ten boards finished within the deadline and received valid KiCad scores. **All ten SES files are byte-for-byte identical to the frozen terminal prototype**, including every normally completed full-run gain and the five regression controls. Local source hashes match the tested workbench source. This is strong extraction evidence, not a second 751-board run or a speed claim.

[Per-board verification](routing-quality-artifacts/after-optimization-repair/cleanup-validation/verification.json) and [test/source/binary provenance](routing-quality-artifacts/after-optimization-repair/cleanup-validation/validation-provenance.json).

## Remaining work before considering a merge

Review the small benefit against code complexity and account for any changes between the evaluated base and current main. A branch push is not a merge or an assertion that these outstanding review questions are settled. Further work should improve genuine difficult cases rather than tune to deadline snapshots or dismiss normal-board regressions as outliers.
