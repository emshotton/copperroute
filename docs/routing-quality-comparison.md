# Routing-quality experiment comparison

Snapshot: 2026-09-09 UTC. Full corpus has 845 selected boards. Local tables use 11 KiCad fixtures, a 120-second limit, and are not directly comparable to full-corpus runs.

CPU is summed routing CPU time, including attempts without a referee score. RAM is average / maximum reported per-board peak RSS in MB, not aggregate host RAM or referee memory. Connected means zero unrouted connections; it does not imply a clean DRC. Unfinished means a scored board with one or more unrouted connections. Unscored results are excluded from connection and DRC totals. DRC here means the benchmark routing-violation metric; solder-mask counts are listed separately below. Raw totals include every successfully scored board, so use matched-board deltas when coverage differs.

## Workbench

| Experiment | Attempts | CPU h | RAM avg / max MB | Unrouted | Routing DRC | Connected | Unfinished | Unscored | Clean |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Rust baseline | 845 | 13.626 | 26.2 / 453.4 | 8,216 | 20,216 | 546 | 295 | 4 | 501 |
| Java reference | 845 | 36.192 | 1215.2 / 2056.5 | 14,788 | 31,793 | 406 | 438 | 1 | 272 |
| Landed neckdown reference | 845 | 13.518 | 26.2 / 453.5 | 7,596 | 20,337 | 548 | 292 | 5 | 498 |
| Nominal clearance | 845 | 12.396 | 25.3 / 372.8 | 7,301 | 20,282 | 602 | 238 | 5 | 550 |
| Footprint identity | 845 | 13.555 | 26.3 / 457.5 | 7,818 | 20,192 | 546 | 294 | 5 | 505 |
| Nominal + smoothing | 845 | 12.107 | 25.4 / 373.0 | 7,095 | 20,228 | 602 | 237 | 6 | 550 |
| Smoothing | 845 | 13.650 | 26.2 / 453.4 | 7,814 | 20,215 | 545 | 295 | 5 | 500 |
| Faster connection checks | 845 | 13.559 | 26.6 / 453.4 | 7,830 | 21,085 | 546 | 296 | 3 | 501 |
| Zero-distance via guard | 845 | 13.860 | 25.7 / 453.5 | 8,037 | 20,210 | 545 | 295 | 5 | 500 |

### Referee breakdown

| Run / candidate | Referee | Attempts | CPU h | RAM avg / max MB | Unrouted | Routing DRC | Mask errors | Connected / unfinished / unscored |
|---|---|---:|---:|---:|---:|---:|---:|---|
| full-java-vs-head-636e039 / rs-main | kicad | 740 | 10.171 | 23.4 / 453.4 | 5,570 | 921 | 13,748 | 519 / 221 / 0 |
| full-java-vs-head-636e039 / rs-main | java-drc | 105 | 3.455 | 45.8 / 325.9 | 2,646 | 19,295 | 0 | 27 / 74 / 4 |
| full-java-vs-head-636e039 / java-current | kicad | 740 | 29.799 | 1211.0 / 2018.3 | 11,043 | 9,931 | 13,771 | 382 / 358 / 0 |
| full-java-vs-head-636e039 / java-current | java-drc | 105 | 6.394 | 1245.1 / 2056.5 | 3,745 | 21,862 | 0 | 24 / 80 / 1 |
| rs-union / rs-main | kicad | 740 | 10.020 | 23.4 / 453.5 | 4,998 | 1,042 | 13,823 | 521 / 218 / 1 |
| rs-union / rs-main | java-drc | 105 | 3.498 | 46.5 / 326.0 | 2,598 | 19,295 | 0 | 27 / 74 / 4 |
| quality-nominal-full / nominal | kicad | 740 | 9.052 | 22.2 / 372.8 | 4,903 | 920 | 14,081 | 567 / 172 / 1 |
| quality-nominal-full / nominal | java-drc | 105 | 3.344 | 47.7 / 325.8 | 2,398 | 19,362 | 0 | 35 / 66 / 4 |
| quality-images-full / images | kicad | 740 | 10.080 | 23.3 / 457.5 | 5,245 | 896 | 13,769 | 519 / 220 / 1 |
| quality-images-full / images | java-drc | 105 | 3.475 | 47.5 / 325.8 | 2,573 | 19,296 | 0 | 27 / 74 / 4 |
| quality-nominal-smoothing-full / nominal-smoothing | kicad | 740 | 8.844 | 22.5 / 373.0 | 4,759 | 914 | 14,057 | 567 / 172 / 1 |
| quality-nominal-smoothing-full / nominal-smoothing | java-drc | 105 | 3.263 | 46.7 / 326.2 | 2,336 | 19,314 | 0 | 35 / 65 / 5 |
| quality-smoothing-full / smoothing | kicad | 740 | 10.154 | 23.2 / 453.4 | 5,230 | 920 | 13,734 | 518 / 221 / 1 |
| quality-smoothing-full / smoothing | java-drc | 105 | 3.496 | 48.2 / 325.9 | 2,584 | 19,295 | 0 | 27 / 74 / 4 |
| quality-census-full / census | kicad | 740 | 10.063 | 23.6 / 453.4 | 5,389 | 937 | 13,742 | 519 / 221 / 0 |
| quality-census-full / census | java-drc | 105 | 3.496 | 48.6 / 326.1 | 2,441 | 20,148 | 0 | 27 / 75 / 3 |
| quality-via-progress-full / via-progress | kicad | 740 | 10.242 | 22.9 / 453.5 | 5,370 | 915 | 13,735 | 518 / 221 / 1 |
| quality-via-progress-full / via-progress | java-drc | 105 | 3.618 | 45.8 / 325.3 | 2,667 | 19,295 | 0 | 27 / 74 / 4 |

## Big EPYC server

All six cohorts complete; updated after 2026-09-09 04:39 UTC.

| Experiment | Attempts | CPU h | RAM avg / max MB | Unrouted | Routing DRC | Connected | Unfinished | Unscored | Clean |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Rust baseline | 845 | 11.025 | 25.5 / 451.9 | 7,668 | 20,218 | 546 | 295 | 4 | 501 |
| Faster connection checks | 845 | 10.969 | 25.9 / 452.0 | 6,932 | 21,074 | 546 | 296 | 3 | 501 |
| Zero-distance via guard | 845 | 11.001 | 25.6 / 452.0 | 7,604 | 20,218 | 546 | 295 | 4 | 501 |
| Via projection guard | 845 | 10.879 | 25.6 / 451.9 | 7,528 | 20,228 | 548 | 293 | 4 | 503 |
| Nominal + smoothing + connection checks | 845 | 9.741 | 24.8 / 370.9 | 6,312 | 20,281 | 603 | 238 | 4 | 551 |
| Smoothing + empty-proposal guard | 845 | 11.075 | 25.6 / 449.7 | 7,631 | 20,204 | 545 | 296 | 4 | 500 |

### Referee breakdown

| Run / candidate | Referee | Attempts | CPU h | RAM avg / max MB | Unrouted | Routing DRC | Mask errors | Connected / unfinished / unscored |
|---|---|---:|---:|---:|---:|---:|---:|---|
| quality-epyc-full-01 / base | kicad | 740 | 8.196 | 22.0 / 451.9 | 5,406 | 922 | 15,045 | 519 / 221 / 0 |
| quality-epyc-full-01 / base | java-drc | 105 | 2.828 | 51.0 / 325.3 | 2,262 | 19,296 | 0 | 27 / 74 / 4 |
| quality-epyc-full-01 / census | kicad | 740 | 8.126 | 22.3 / 452.0 | 4,698 | 925 | 15,032 | 519 / 221 / 0 |
| quality-epyc-full-01 / census | java-drc | 105 | 2.843 | 51.2 / 324.7 | 2,234 | 20,149 | 0 | 27 / 75 / 3 |
| quality-epyc-full-01 / via-progress | kicad | 740 | 8.169 | 22.0 / 452.0 | 5,364 | 922 | 15,047 | 519 / 221 / 0 |
| quality-epyc-full-01 / via-progress | java-drc | 105 | 2.832 | 51.4 / 325.0 | 2,240 | 19,296 | 0 | 27 / 74 / 4 |
| quality-epyc-full-01 / via-projection | kicad | 740 | 8.045 | 22.1 / 451.9 | 5,267 | 932 | 15,205 | 521 / 219 / 0 |
| quality-epyc-full-01 / via-projection | java-drc | 105 | 2.834 | 50.8 / 325.3 | 2,261 | 19,296 | 0 | 27 / 74 / 4 |
| quality-epyc-full-01 / nominal-smoothing-census | kicad | 740 | 7.107 | 21.3 / 370.9 | 4,243 | 919 | 15,441 | 568 / 172 / 0 |
| quality-epyc-full-01 / nominal-smoothing-census | java-drc | 105 | 2.635 | 50.1 / 324.1 | 2,069 | 19,362 | 0 | 35 / 66 / 4 |
| quality-epyc-full-01 / smoothing-noop | kicad | 740 | 8.252 | 22.1 / 449.7 | 5,381 | 908 | 15,040 | 518 / 222 / 0 |
| quality-epyc-full-01 / smoothing-noop | java-drc | 105 | 2.823 | 51.0 / 324.7 | 2,250 | 19,296 | 0 | 27 / 74 / 4 |

## Local 11-board experiments

| Experiment | Attempts | CPU h | RAM avg / max MB | Unrouted | Routing DRC | Connected | Unfinished | Unscored | Clean |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Baseline | 11 | 0.082 | 33.5 / 79.8 | 250 | 40 | 6 | 5 | 0 | 6 |
| Faster connection checks | 11 | 0.086 | 34.8 / 109.1 | 237 | 40 | 6 | 5 | 0 | 6 |
| Connection checks + both via guards | 11 | 0.084 | 31.9 / 72.1 | 252 | 40 | 6 | 5 | 0 | 6 |
| Nominal + smoothing + connection checks | 11 | 0.079 | 37.5 / 138.5 | 226 | 38 | 8 | 3 | 0 | 8 |
| Footprint identity | 11 | 0.085 | 35.5 / 97.5 | 243 | 40 | 6 | 5 | 0 | 6 |
| Footprint identity + smoothing | 11 | 0.091 | 38.2 / 134.6 | 231 | 40 | 6 | 5 | 0 | 5 |
| Nominal clearance | 11 | 0.081 | 37.7 / 137.0 | 225 | 38 | 8 | 3 | 0 | 8 |
| Nominal + smoothing | 11 | 0.079 | 37.0 / 136.1 | 226 | 38 | 8 | 3 | 0 | 8 |
| Nominal clearance around pins only | 11 | 0.083 | 39.7 / 154.2 | 229 | 39 | 7 | 4 | 0 | 6 |
| Smoothing | 11 | 0.088 | 39.1 / 134.8 | 231 | 40 | 6 | 5 | 0 | 5 |
| Smoothing + empty-proposal guard | 11 | 0.089 | 34.5 / 84.6 | 234 | 40 | 5 | 6 | 0 | 5 |
| Zero-distance via guard | 11 | 0.087 | 34.2 / 95.0 | 239 | 40 | 6 | 5 | 0 | 6 |
| Via projection guard | 11 | 0.086 | 34.0 / 92.6 | 243 | 40 | 6 | 5 | 0 | 6 |

### Referee breakdown

| Run / candidate | Referee | Attempts | CPU h | RAM avg / max MB | Unrouted | Routing DRC | Mask errors | Connected / unfinished / unscored |
|---|---|---:|---:|---:|---:|---:|---:|---|
| quality-local-base / base | kicad | 11 | 0.082 | 33.5 / 79.8 | 250 | 40 | 0 | 6 / 5 / 0 |
| quality-local-census / census | kicad | 11 | 0.086 | 34.8 / 109.1 | 237 | 40 | 0 | 6 / 5 / 0 |
| quality-local-census-via-guards / census-via-guards | kicad | 11 | 0.084 | 31.9 / 72.1 | 252 | 40 | 0 | 6 / 5 / 0 |
| quality-local-combined-census / nominal-smoothing-census | kicad | 11 | 0.079 | 37.5 / 138.5 | 226 | 38 | 0 | 8 / 3 / 0 |
| quality-local-images / images | kicad | 11 | 0.085 | 35.5 / 97.5 | 243 | 40 | 0 | 6 / 5 / 0 |
| quality-local-images-smoothing / images-smoothing | kicad | 11 | 0.091 | 38.2 / 134.6 | 231 | 40 | 0 | 6 / 5 / 0 |
| quality-local-nominal / nominal | kicad | 11 | 0.081 | 37.7 / 137.0 | 225 | 38 | 0 | 8 / 3 / 0 |
| quality-local-nominal-smoothing / nominal-smoothing | kicad | 11 | 0.079 | 37.0 / 136.1 | 226 | 38 | 0 | 8 / 3 / 0 |
| quality-local-pin / pin | kicad | 11 | 0.083 | 39.7 / 154.2 | 229 | 39 | 0 | 7 / 4 / 0 |
| quality-local-smoothing / smoothing | kicad | 11 | 0.088 | 39.1 / 134.8 | 231 | 40 | 0 | 6 / 5 / 0 |
| quality-local-smoothing-noop / smoothing-noop | kicad | 11 | 0.089 | 34.5 / 84.6 | 234 | 40 | 0 | 5 / 6 / 0 |
| quality-local-via-progress / via-progress | kicad | 11 | 0.087 | 34.2 / 95.0 | 239 | 40 | 0 | 6 / 5 / 0 |
| quality-local-via-projection / via-projection | kicad | 11 | 0.086 | 34.0 / 92.6 | 243 | 40 | 0 | 6 / 5 / 0 |

## Interpretation

- Workbench via-progress remains a partial snapshot. EPYC smoothing-noop has now completed; the overview table is updated, but the original detailed referee snapshot below it remains partial.
- The cumulative three-candidate EPYC batch started at 2026-09-09 04:39:30 UTC.
- Census newly scores issue756-tomu-fpga8: 11 unrouted / 853 Java violations. Matched-board deltas exclude this newly scored board.
- Workbench nominal, footprint identity, and smoothing have a newly unscored Djinn output. Nominal+smoothing also loses issue070. Excluding these failed outputs from totals must not be interpreted as successful connection gains.
- EPYC baseline teensy-weather-badge and projection DoroidOscillo display/import failures were repaired by rescoring unchanged SES output. Full original cells and successful retry reports were preserved.

## Workbench matched-board change summary

| Candidate | Referee | Pairs | U improved / regressed | Delta U | DRC gainers | Delta routing DRC | CPU ratio |
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

## EPYC matched-board change summary

| Candidate | Referee | Pairs | U improved / regressed | Delta U | DRC gainers | Delta routing DRC | CPU ratio |
|---|---|---:|---:|---:|---:|---:|---:|
| census | kicad | 740 | 5 / 5 | -708 | 1 | +3 | 0.9914 |
| census | java-drc | 101 | 4 / 2 | -39 | 0 | +0 | 0.9701 |
| via-progress | kicad | 740 | 3 / 2 | -42 | 0 | +0 | 0.9967 |
| via-progress | java-drc | 101 | 5 / 1 | -22 | 0 | +0 | 1.0011 |
| via-projection | kicad | 740 | 6 / 3 | -139 | 2 | +10 | 0.9815 |
| via-projection | java-drc | 101 | 3 / 3 | -1 | 0 | +0 | 1.0019 |


Nominal+smoothing+census has now completed on EPYC. Its matched KiCad results are 740 boards, 129 U improvements / 50 regressions, -1,163 U, 31 DRC gainers, -3 routing violations, CPU ratio 0.86705, and +396 solder-mask errors. Java results are 101 paired boards, 36 improvements / 3 regressions, -193 U, one DRC gainer, +66 violations, CPU ratio 0.93149.


## EPYC cumulative validation (completed)

Same 841 scored boards; newly scored fixtures are separate.

| Parent → change | Referee | Pairs | U improved / regressed | ΔU | DRC gainers | ΔV | CPU ratio |
|---|---|---:|---:|---:|---:|---:|---:|
| census → census-via-progress | kicad | 740 | 3 / 4 | -25 | 0 | +0 | 0.99621 |
| census → census-via-progress | java-drc | 101 | 3 / 3 | -14 | 0 | +0 | 0.99855 |
| census-via-progress → census-via-guards | kicad | 740 | 10 / 3 | -137 | 2 | +10 | 0.96901 |
| census-via-progress → census-via-guards | java-drc | 101 | 2 / 1 | -6 | 0 | +0 | 0.99199 |
| census → census-via-guards | kicad | 740 | 10 / 6 | -162 | 2 | +10 | 0.96534 |
| census → census-via-guards | java-drc | 101 | 4 / 3 | -20 | 0 | +0 | 0.99056 |


Workbench standalone zero-distance run is complete. Seven failed KiCad
imports were successfully rescored from byte-identical saved SES; original
failed referee artifacts are preserved. Djinn alone is newly unscored.
Matched KiCad 739: 3 improved / 12 regressed, +196 U, 2 DRC gainers /
-6 routing V, CPU ratio 1.01658. Java 101: 5 improved / 8 regressed, +21 U,
no DRC gainers / unchanged V, CPU ratio 1.04713. This negative full run is
retained alongside the positive EPYC results. The affected-board matched
repeat is complete: 12 scored pairs, 5 U improvements / 2 regressions,
+7 U and +5 V; Djinn newly scores and is separate. ReSDMAC and decelerator
remain worse. A separate SRAM-bank pair finishes identically at 108 U /
0 V. These observations investigate, rather than erase, the negative full run.


## Final latest-main comparison (completed)

Baseline main 6886640, change cd539fd (benchmark label identifies its pre-commit source), EPYC, 845 attempts per candidate. Four baseline Java failures are excluded from the matched quality totals. One of those fixtures is newly scored by the combined candidate (11 U / 853 V), reported separately. No newly unscored boards after verified HellScribe referee repair.

| Comparison | Referee | Pairs | U improved / regressed | ΔU | DRC gainers | Δ routing V | CPU ratio |
|---|---|---:|---:|---:|---:|---:|---:|
| main → zero-only | kicad | 740 | 7 / 5 | -729 | 1 | +3 | 0.98060 |
| main → zero-only | java-drc | 101 | 4 / 4 | -48 | 0 | -1 | 0.96795 |
| zero-only → change | kicad | 740 | 12 / 3 | -153 | 3 | +12 | 0.96905 |
| zero-only → change | java-drc | 101 | 3 / 1 | -5 | 0 | +0 | 0.99094 |
| main → change | kicad | 740 | 11 / 4 | -882 | 4 | +15 | 0.95025 |
| main → change | java-drc | 101 | 5 / 3 | -53 | 0 | -1 | 0.95918 |

Combined versus main: 935 fewer U / 14 more routing V across 841 matched scored boards. Two new routing-clean boards, no clean losses. KiCad mask errors +151, all-type errors +166. The official regression gate fails with eight quality losses; full output is in [routing-quality-pr-performance.md](routing-quality-pr-performance.md). Single-repetition CPU observations do not establish a statistical speedup.
