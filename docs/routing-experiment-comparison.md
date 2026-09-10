# Routing experiment comparison

Snapshot: 2026-09-09. Negative deltas mean fewer unrouted connections or DRC reports. CPU ratio is candidate/control, not a statistically established speedup. Comparisons use different baselines: do not add their deltas. Copper DRC excludes solder-mask reports. All current populations use KiCad. Historical Java DRC is intentionally excluded.

Recent comparisons below are reproduced from saved comparison JSON. PCBench and local KiCad fixtures are separate. Pilot files are selected samples, not full-corpus results. A dash means the comparison did not summarize that metric.

| Experiment / comparison file | Population | Boards | Δ unrouted | U better/worse | Δ copper DRC | Copper gainers | Δ mask | CPU ratio | Completed control→candidate | Connected control→candidate | Median RSS MB | Max RSS MB |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|---|---|---|
| board-minimum-full-comparison | pcbench | 740 | -336 | 16/0 | -29 | 1 | -1 | 0.9444 | 710→717 | 588→589 | 12.1→12.1 | 369.4→370.9 |
| board-minimum-full-comparison | local | 11 | +0 | 0/0 | +0 | 0 | +0 | 0.9998 | 10→10 | 8→8 | 13.6→12.1 | 71.3→72.8 |
| board-minimum-pilot-comparison | pcbench | 6 | -1 | 1/0 | -31 | 0 | -1 | 1.0535 | 5→5 | 3→3 | 22.4→23.9 | 99.8→98.4 |
| bundle-mask-comparison | pcbench | 740 | -304 | 55/22 | +31 | 28 | -303 | 0.9307 | 711→718 | 588→595 | 12.1→12.1 | 369.4→366.4 |
| bundle-mask-comparison | local | 11 | +4 | 0/2 | -1 | 0 | +0 | 0.9089 | 10→10 | 8→7 | 12.1→13.6 | 73.0→76.5 |
| bundle-order-comparison | pcbench- | 740 | +76 | 54/51 | +48 | 22 | -77 | 0.9693 | — | — | — | — |
| bundle-order-comparison | kicad- | 11 | -10 | 2/2 | -1 | 1 | +0 | 1.0267 | — | — | — | — |
| clearance-boundary-pilot-comparison | pcbench | 6 | +1 | 3/1 | -11 | 1 | +32 | 1.0118 | 6→6 | 1→1 | 53.8→57.75 | 85.5→85.7 |
| corridor-coherent-comparison | pcbench | 740 | -245 | 45/14 | -1 | 14 | -113 | 0.9418 | 711→717 | 588→598 | 12.1→12.1 | 369.4→369.4 |
| corridor-coherent-comparison | local | 11 | +4 | 0/2 | +1 | 1 | +0 | 1.0058 | 10→10 | 8→7 | 13.6→12.0 | 69.4→120.9 |
| corridor-minimum-comparison | pcbench | 740 | -172 | 44/13 | -3 | 13 | -64 | 0.9401 | 711→717 | 588→598 | 12.1→12.1 | 370.9→369.4 |
| corridor-minimum-comparison | local | 11 | +4 | 0/2 | +1 | 1 | +0 | 1.0078 | 10→10 | 8→7 | 12.1→12.1 | 69.8→118.8 |
| gap-comparison | pcbench- | 740 | -559 | 124/50 | -34 | 28 | -4287 | 0.8583 | — | — | — | — |
| gap-comparison | kicad- | 11 | -15 | 3/0 | -2 | 1 | +0 | 0.9179 | — | — | — | — |
| mask-review-comparison | pcbench | 740 | -1150 | 140/38 | -48 | 26 | -4244 | 0.8730 | — | — | — | — |
| mask-review-comparison | local | 11 | -15 | 3/0 | -2 | 1 | +0 | 0.9189 | — | — | — | — |
| mask-via-comparison | pcbench | 740 | -997 | 139/39 | -55 | 26 | -4382 | 0.8721 | — | — | — | — |
| mask-via-comparison | local | 11 | -15 | 3/0 | -2 | 1 | +0 | 0.9198 | — | — | — | — |
| neckdown-minimum-full-comparison | pcbench | 740 | -317 | 15/0 | -2 | 2 | -47 | 0.9422 | 710→717 | 588→588 | 12.1→12.1 | 366.4→370.9 |
| neckdown-minimum-full-comparison | local | 11 | +0 | 0/0 | +0 | 0 | +0 | 0.9967 | 10→10 | 8→8 | 12.0→13.6 | 71.6→74.6 |
| neckdown-minimum-pilot-comparison | pcbench | 4 | +0 | 0/0 | -1 | 0 | -2 | 0.9935 | 4→4 | 0→0 | 51.9→53.1 | 83.9→85.4 |
| pad-copper-full-comparison | pcbench | 740 | -289 | 17/3 | -115 | 5 | -35 | 0.9499 | 710→716 | 588→588 | 12.1→12.1 | 370.9→372.3 |
| pad-copper-full-comparison | local | 11 | +0 | 0/0 | +0 | 0 | +0 | 0.9977 | 10→10 | 8→8 | 13.5→10.6 | 76.1→78.3 |
| pad-copper-matched-comparison | pcbench | 740 | -459 | 17/3 | -118 | 4 | +39 | 0.9498 | 710→716 | 588→589 | 12.1→12.1 | 367.9→374.3 |
| pad-copper-matched-comparison | local | 11 | +0 | 0/0 | +0 | 0 | +0 | 0.9938 | 10→10 | 8→8 | 13.6→13.6 | 66.0→79.1 |
| pad-copper-pilot-comparison | pcbench | 5 | +6 | 0/2 | -46 | 1 | -29 | 1.1866 | 5→5 | 2→2 | 26.7→27.1 | 66.3→375.8 |
| rollback-comparison | pcbench- | 740 | -264 | 64/49 | -82 | 13 | +12 | 0.9781 | — | — | — | — |
| rollback-comparison | kicad- | 11 | -1 | 1/3 | -3 | 0 | +0 | 1.1338 | — | — | — | — |

## Earlier EPYC full experiment: KiCad subset only

These are absolute totals on 740 KiCad boards, compared against the baseline in the same table. Historical mixed-referee totals are not used.

| Candidate | Unrouted | Copper DRC | Mask reports | CPU seconds | Mean peak RSS MB | Max peak RSS MB | Connected / still disconnected |
|---|---:|---:|---:|---:|---:|---:|---|
| base | 5406 | 922 | 15045 | 29506.12 | 22.01 | 451.9 | 519 / 221 |
| census | 4698 | 925 | 15032 | 29252.74 | 22.32 | 452.0 | 519 / 221 |
| via-progress | 5364 | 922 | 15047 | 29408.93 | 22.01 | 452.0 | 519 / 221 |
| via-projection | 5267 | 932 | 15205 | 28961.31 | 22.09 | 451.9 | 521 / 219 |
| nominal-smoothing-census | 4243 | 919 | 15441 | 25583.40 | 21.29 | 370.9 | 568 / 172 |
| smoothing-noop | 5381 | 908 | 15040 | 29707.90 | 22.11 | 449.7 | 518 / 222 |

## Interpretation

- Completed on both sides: board-minimum −1 unrouted/−31 copper; pad-local +4/−119 in both full repetitions; corridor with board minima −46/−3; minimum-width retry 0/−1.
- Boundary contact had a six-board completed pilot of +1 unrouted/−11 copper, but its subsequent full completed comparison was −2 unrouted/+13 copper. Ordinary copper gainers include saiboard, 96boards Sensors, azalea and rjw57. Held after the negative full measurement.
- Identical SES outputs account for some mask-count differences. Repeating KiCad on an identical saved BLDC PCB gave 202 then 229 mask reports; copper/width counts stayed stable. Reported mask deltas are retained, but small changes are not established routing effects.
- A completed run can still have unrouted connections. Connected means zero unrouted.
- Current detailed comparisons are retained in /tmp/quality-epyc-bootstrap; raw experiment results are mirrored to workbench. Historical mechanisms and local diagnostics are recorded in routing-quality-log.md.

## Post-merge measurements (2026-09-10 UTC)

These additions use the same KiCad-only scoring and retain all boards. Pilot samples are deliberately selected and cannot establish corpus-wide benefit. CPU ratios are descriptive. Completed-output results below separate routing changes from deadline effects and mask variability.

| Experiment | Population | Boards | Δ U | U better/worse | Δ copper | Copper gainers | Δ reported mask | CPU ratio | Completed | Connected | Median RSS MB | Max RSS MB |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|---|---|---|
| Pad floors after main merge | pcbench | 740 | -302 | 16/3 | -117 | 4 | -14 | 0.9499 | 709→716 | 588→588 | 12.1→12.1 | 366.4→368.7 |
| Pad floors after main merge | local | 11 | +0 | 0/0 | +0 | 0 | +0 | 1.0040 | 10→10 | 8→8 | 12.1→13.6 | 68.3→76.9 |
| Image identity before pad merge | pcbench | 740 | -346 | 25/6 | -49 | 3 | -15 | 0.9383 | 710→718 | 588→590 | 12.1→12.1 | 369.4→373.9 |
| Image identity before pad merge | local | 11 | +0 | 0/0 | +0 | 0 | +0 | 1.0063 | 10→10 | 8→8 | 12.1→12.1 | 71.7→70.3 |
| Reference correction vs pad control | pcbench | 740 | -360 | 15/0 | -24 | 1 | -365 | 0.9390 | 709→717 | 588→589 | 12.1→12.1 | 368.9→370.9 |
| Reference correction vs pad control | local | 11 | +0 | 0/0 | +0 | 0 | +0 | 0.9941 | 10→10 | 8→8 | 12.1→13.6 | 72.0→74.0 |
| Pad floors + reference correction vs main | pcbench | 740 | -384 | 15/4 | -139 | 5 | -339 | 0.9449 | 709→717 | 588→589 | 12.1→12.1 | 370.2→370.9 |
| Pad floors + reference correction vs main | local | 11 | +0 | 0/0 | +0 | 0 | +0 | 1.0018 | 10→10 | 8→8 | 13.6→13.6 | 76.2→74.0 |
| Images after pad merge vs current main | pcbench | 740 | -288 | 25/4 | -46 | 3 | -766 | 0.9346 | 710→718 | 588→590 | 12.1→12.1 | 370.9→372.7 |
| Images after pad merge vs current main | local | 11 | +0 | 0/0 | +0 | 0 | +0 | 1.0008 | 10→10 | 8→8 | 13.6→12.1 | 74.9→78.0 |
| Images + pads pilot vs pads | pcbench | 8 | +4 | 0/4 | -15 | 2 | -5 | 1.0286 | 8→8 | 4→3 | 32.15→34.55 | 81.9→80.1 |
| Terminal-envelope pilot vs corridor | pcbench | 6 | +5 | 1/4 | -1 | 0 | +4 | 1.0547 | 6→6 | 2→1 | 49.75→52.45 | 67.5→69.7 |
| Terminal-envelope pilot vs corridor | local | 1 | -2 | 1/0 | -2 | 0 | +0 | 1.0067 | 1→1 | 0→0 | 33.2→32.0 | 33.2→32.0 |
| Local-hole pilot vs control | pcbench | 4 | +1 | 0/1 | -3 | 0 | +0 | 0.9976 | 4→4 | 3→2 | 44.75→48.150000000000006 | 72.0→73.5 |
| Local-hole full vs same-binary control | pcbench | 740 | -433 | 16/3 | -8 | 2 | +639 | 0.9481 | 712→717 | 590→589 | 12→12 | 373.9→372.4 |
| Local-hole full vs same-binary control | local | 11 | +0 | 0/0 | +0 | 0 | +0 | 0.9975 | 10→10 | 8→8 | 13.5→13.5 | 78.2→72.8 |
| Local-hole full vs main 7d33cef | pcbench | 740 | -464 | 15/3 | -8 | 2 | +34 | 0.9478 | 709→717 | 590→589 | 12.1→12 | 370.9→372.4 |
| Local-hole full vs main 7d33cef | local | 11 | +0 | 0/0 | +0 | 0 | +0 | 1.0038 | 10→10 | 8→8 | 13.6→13.5 | 75.2→72.8 |
| Corridor current-main vs same-binary control | pcbench | 740 | -340 | 46/15 | +18 | 10 | -66 | 0.9385 | 710→718 | 590→601 | 12.1→12.1 | 373.9→368.7 |
| Corridor current-main vs same-binary control | local | 11 | +4 | 0/2 | +1 | 1 | +0 | 1.0119 | 10→10 | 8→7 | 13.6→13.5 | 72.2→121.4 |
| Corridor vs main 7d33cef | pcbench | 740 | -375 | 46/14 | +20 | 10 | -86 | 0.9416 | 709→718 | 590→601 | 12.1→12.1 | 370.9→368.7 |
| Corridor vs main 7d33cef | local | 11 | +4 | 0/2 | +1 | 1 | +0 | 1.0184 | 10→10 | 8→7 | 13.6→13.5 | 76.7→121.4 |
| Shove guard vs main54d4e80 | pcbench | 740 | -1 | 6/2 | +1 | 1 | -418 | 1.0013 | 709→709 | 589→589 | 12.1→12.1 | 372.4→372.4 |
| Shove guard vs main54d4e80 | local | 11 | +0 | 0/0 | +0 | 0 | +0 | 1.0062 | 10→10 | 8→8 | 12→12.1 | 75.8→71.5 |
| Corridor vs main54d4e80 (five-way) | pcbench | 740 | -47 | 39/18 | +17 | 8 | -17 | 1.0006 | 709→713 | 589→599 | 12.1→12.1 | 372.4→375.4 |
| Corridor vs main54d4e80 (five-way) | local | 11 | +4 | 0/2 | +1 | 1 | +0 | 1.0142 | 10→10 | 8→7 | 12→13.6 | 75.8→96.1 |
| Corridor + guard vs main54d4e80 | pcbench | 740 | -282 | 45/16 | -1 | 8 | -74 | 0.9432 | 709→717 | 589→599 | 12.1→12.1 | 372.4→375.4 |
| Corridor + guard vs main54d4e80 | local | 11 | +4 | 0/2 | +1 | 1 | +0 | 1.0194 | 10→10 | 8→7 | 12→13.5 | 75.8→105.1 |
| Guard added to corridor | pcbench | 740 | -235 | 14/4 | -18 | 3 | -57 | 0.9426 | 713→717 | 599→599 | 12.1→12.1 | 375.4→375.4 |
| Guard added to corridor | local | 11 | +0 | 0/0 | +0 | 0 | +0 | 1.0051 | 10→10 | 7→7 | 13.6→13.5 | 96.1→105.1 |
| Final coupled guidance vs main54d4e80 | pcbench | 740 | -190 | 44/16 | -1 | 8 | -107 | 0.9431 | 709→716 | 589→599 | 12→12.1 | 370.9→372.4 |
| Final coupled guidance vs main54d4e80 | local | 11 | +4 | 0/2 | +1 | 1 | +0 | 1.0108 | 10→10 | 8→7 | 13.5→12.1 | 67.3→124.5 |
| Final coupled guidance vs disabled control | pcbench | 740 | -155 | 44/17 | -3 | 8 | -100 | 0.9410 | 710→716 | 589→599 | 12.1→12.1 | 372.4→372.4 |
| Final coupled guidance vs disabled control | local | 11 | +4 | 0/2 | +1 | 1 | +0 | 1.0045 | 10→10 | 8→7 | 13.6→12.1 | 83→124.5 |

**Completed-output interpretation:** PR29 with corrected references gives +1 U/−140 copper versus pre-merge main; reference correction alone gives −3 U/−21 copper with no completed U/copper regressions. PR31 on top gives −15 U/−46 copper. Together they give −14 U/−186 copper against d84ac9e on 718 completed pairs. PR29 and PR31 are merged; those measurements used main7d33cef. Current main is54d4e80 after PR32. Large reported mask deltas mostly occur on identical output and are not claimed as routing improvements.

The terminal-envelope pilot adds +3 U/−3 copper versus current corridor guidance across seven completed boards; held, not landed. The local-hole pilot adds +1 U/−3 copper across four completed boards. Its full current-main run has now completed: 0 U/−9 copper on 719 completed main/candidate pairs (722 for same-binary control), three copper-improved boards and no completed copper gainers. EncoderBoard +1 U and ESP32 −1 U are retained. All completed mask changes occur on identical SES outputs. PR32 merged as54d4e80 with the failed seven-loss gate and ordinary EncoderBoard regression documented. The original corridor current-main retest completed: −45U/+22Cu across 720 completed control pairs. OpenHardwareExG accounts +2U/+24Cu, including 12 shorts and eight clearance violations; its original is 0U/0Cu. PR27 remains draft, with 107-loss generated gate retained. All 2253 KiCad referees succeeded. Updated PR branch43d9a18 matches measured600source/data files and passes2592tests/77ignored. EncoderBoard20-pass diagnostic reaches0U0Cu on both control/holes, while its10-pass regression remains counted.

The five-way shove-revalidation run scored all3,755cases successfully. Main/control719completedoutputs are identical. Guard alone is +6U/0 copper on719completedpairs; combined guidance+guard is **−36U/0 net copper**, with29U-improved/13U-regressed boards and7copper-improved/7copper-gaining boards. PCBench contributes−39U/−1copper, local+3U/+1copper. Guard versus corridor is+8U/−21copper on723completedpairs. Combined/main492identicaloutputs accountfor−7mask;227changedoutputs give−36U/0copper/−91mask. Raw deadline and single-run CPU differences are not causal performance claims. The generated combined gate fails112qualitylosses. Boatcontrol+6U and uSKY+3U are retained; replay found other-net obstacles in each captured rejected path. VC4000−2U/+2copper involves omitted B.Cu text and is now recorded as an input-fidelity outlier, still counted. Full source: routing-quality-artifacts/shove-revalidation/comparisons.json.

Final configuration now couples the guidance switch to its safety guard automatically. Its separate three-candidate full validation is running as quality-epyc-shove-coupled-full-01; do not reuse the earlier totals as completed qualification of the new source. The scratch-source workspace passed2599top-level tests plus3childchecks,77ignored0failed; actual PR27-branch fresh suite is running.

Final coupled validation completed:2253successfulKiCadcases. Main719completedpairs and control720pairs both give−36U/0copper. All726completedoutputs shared with previous combined experiment are identical. Actual PR27 source passes2599top-level tests plus3childchecks,77ignored0failed;602source/datahashesmatch. Generated gate112losses retained. Guidance stays opt-in and automatically enables its guard. Final report: corridor-routing-report.md.
