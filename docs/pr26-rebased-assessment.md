# PR #26: neckdown rebase assessment

**Keep draft.** The unchanged neckdown relaxation has been reapplied cleanly onto main `736d02d01912f41ec9ff6ff48017bc93dda0b5bf`, which includes default corridor guidance. It improves connectivity, but its copper DRC regressions affect ordinary boards and are not concentrated on the known artwork outliers. No further routing behavior was changed for this evaluation.

Full run `quality-epyc-pr26-rebased-full-01`: 740 PCBench plus 11 local KiCad fixtures, two candidates, ten passes, 300 seconds, one routing thread, 192 jobs on the EPYC server. Both use default corridor guidance, the same pad metadata and project minimum constraints, and KiCad 10.0.4. No Java referee. All 1,502 scores are valid; 43 initial referee failures were retried on unchanged SES outputs.

## Quality and CPU

Negative deltas are reductions. DRC means the benchmark’s copper/routing violation count; solder mask is reported separately. U better/worse counts connectivity changes. CPU is candidate/baseline and is an observation from one repetition, not evidence of a speed improvement.

| Subset | Boards | U better / worse | Δ unrouted | DRC gainers | Δ copper DRC | CPU ratio |
|---|---:|---:|---:|---:|---:|---:|
| pcbench/all | 740 | 23 / 3 | -197 | 13 | +45 | 0.9474 |
| local/all | 11 | 1 / 0 | -7 | 1 | +7 | 1.0083 |
| pcbench/both_completed | 713 | 11 / 1 | -35 | 9 | +32 | 0.9339 |
| local/both_completed | 10 | 1 / 0 | -7 | 1 | +7 | 1.0132 |

All 751 boards: **4,705 → 4,501 unrouted (−204); 706 → 758 copper violations (+52)**. Connectivity improves on 24 boards and regresses on three; copper improves on four and worsens on 14. Fully connected boards rise 606 → 607. Router COMPLETED counts rise 723 → 726; internal TIMED_OUT counts fall 28 → 25. These internal deadlines are included even when the benchmark’s outer timeout flag is false.

On the 723 boards COMPLETED on both sides: **1,290 → 1,248 unrouted (−42); 613 → 652 copper violations (+39)**. Connectivity improves on 12 boards and regresses on one; copper improves on two and worsens on ten. The remaining −162 U/+13 copper belongs to deadline-affected pairs and is not treated as a stable algorithm result. No outlier board is excluded.

PCBench median/max RSS remains 12.1/373.9 MiB on both candidates. Local median RSS remains 13.6 MiB; maximum is 90.9 → 114.0 MiB. Full CPU totals are 35,080.31 → 33,281.32 seconds; completed-pair totals are 26,695.31 → 24,968.53 seconds. Single-seed contention and timeout differences prevent a causal performance claim.

## Regressor review

All nine PCBench copper-gaining boards in the completed-pair comparison have original ground truth with zero routing DRC and zero unconnected items. The local motorized-opener original has no routing and is not a comparable successful hand-routed reference.

| Board | Δ U | Δ copper | KiCad evidence |
|---|---:|---:|---|
| pwm-2420-lus | −8 | +12 | New 0.200/0.240 mm tracks violate its 0.250 mm minimum. |
| motorizedopener (local) | −7 | +7 | Eight new 0.1874 mm width errors against 0.200 mm minimum; one hole-clearance error removed. |
| oasis_ledboard | −6 | +9 | Nine new vias have 0.400 mm drills against 0.508 mm minimum. Original has no vias and 93.986 mm routing; main lays no traces, candidate uses 216.516 mm and nine vias. |
| BLDC-controller | −1 | +4 | 0.2498 mm tracks against 0.250 mm minimum. |
| LPC2148 Stick and autosave copy | 0 each | +3 each | 0.2498 mm tracks against 0.254 mm minimum. |
| serial_gw_ATMEGA328P | 0 | +2 | 0.125 mm tracks against 0.200 mm minimum. |
| avr_ledprojection-0402 | 0 | +2 | Candidate width findings include 0.2094/0.2498 mm against 0.254 mm minimum. |
| DerKnopf and motor-3xdrv8833 | 0 each | +1 each | 0.1874 mm tracks against 0.200 mm minimum. |
| freeDSP CLASSIC SMD BALANCED | +2 | 0 | Previously fully connected; original is connected with zero routing DRC, 57 vias and 3381.314 mm routing. Main uses 68 vias/4131.287 mm; candidate 64 vias/4178.937 mm. No outlier explanation established. |

These are explicit fabrication-rule violations, not evidence that the original designers did something unusual. The oasis via-size problem is an existing input/rule enforcement gap exposed by successful routing; this patch does not alter the via definition. The exact creation path of each narrow segment has not been instrumented. The patch permits the width ladder below the board floor when a pad requests neckdown; KiCad demonstrates that the resulting permission is too broad for these boards. A future revision should distinguish nominal net-class width from the physical manufacturing minimum and investigate via legality. That revision has not been implemented or measured here.

## Validation and interpretation

`cargo test --workspace` passed: 2,609 checks including isolated child processes, 77 ignored, zero failed. The original failing-first fanout regression passes. No JVM recordings changed. All 602 source/data/test hashes match the frozen server build.

Baseline sanity: against the previous default-corridor evaluation, all 723 mutually completed main outputs are byte-identical, with zero unrouted/copper delta. The referee did not silently collapse violations to zero.

Within this run, 689 completed main/candidate SES outputs are byte-identical: zero U/copper change, but −18 mask findings. All-board mask changes are −28; completed-pair changes are −10. This variation on identical outputs prevents claiming a solder-mask improvement.

The generated benchmark gate reports 24 routing-quality losses; rounded clean-pass rate remains 0.76 → 0.76. Its overall wins/losses also include timing and minor score changes and should not be confused with connectivity counts above.

[Full generated PR summary](routing-quality-artifacts/pr26-rebased/quality-epyc-pr26-rebased-full-01-vs-main.pr.md), [paired comparisons](routing-quality-artifacts/pr26-rebased/quality-epyc-pr26-rebased-full-01-comparisons.json), [regressor evidence](routing-quality-artifacts/pr26-rebased/regressor-audit.json), and [validation record](routing-quality-artifacts/pr26-rebased/validation.txt). Raw sessions, routed boards, referee outputs and logs are backed up on workbench; final reports and original-board evidence also reside on the laptop. The temporary server is no longer needed.
