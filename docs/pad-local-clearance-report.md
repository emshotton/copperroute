# Import local pad copper clearance

KiCad pad and footprint copper-clearance overrides are absent from exported DSN rules. For example, minimal_node_rfm69w requires 300µm around particular pads while DSN permits 250µm. This change imports optional `copperClearance` in native pad JSON (mm), and `copper_um` in the existing DSN pad metadata (µm). Copper floors apply independently of the solder-mask gap cap and round upward. Higher existing clearances remain. This conservatively enforces minima; it does not implement custom-rule precedence or overrides that relax higher net-class clearances.

The browser resolves pad then footprint values. KiCad file versions through 20240201 treat zero as inheritance; later formats preserve explicit zero. Finite negative original values are accepted and do not lower existing minima. Derived metadata floors are nonnegative. Sources: [KiCad parser](https://github.com/KiCad/kicad-source-mirror/blob/10.0.4/pcbnew/pcb_io/kicad_sexpr/pcb_io_kicad_sexpr_parser.cpp), [rule engine](https://github.com/KiCad/kicad-source-mirror/blob/10.0.4/pcbnew/drc/drc_engine.cpp), [pad inheritance](https://github.com/KiCad/kicad-source-mirror/blob/10.0.4/pcbnew/pad.cpp).

## Validation against merged main d84ac9e

`quality-epyc-pad-post-main-01` ran main, same-binary control and pad-copper on all 751 boards: all 2,253 KiCad referee results succeeded. Main was verified unchanged at d84ac9e after the run. Every candidate receives the same project minimum-clearance files; only pad-copper receives the new local pad rules. Settings: 192 jobs, one routing thread, ten passes, 300-second timeout. Results are backed up locally and on workbench.

| Group and baseline | Unrouted delta / improved / regressed boards | Copper delta / boards gaining violations | Reported mask delta / gainers | CPU ratio |
|---|---:|---:|---:|---:|
| PCBench 740 vs main | −302 / 16 / 3 | −117 / 4 | −14 / 8 | 0.94990 |
| Local 11 vs main | 0 / 0 / 0 | 0 / 0 | 0 / 0 | 1.00399 |
| PCBench 740 vs same-binary control | −278 / 16 / 3 | −117 / 4 | −74 / 8 | 0.95004 |
| Local 11 vs same-binary control | 0 / 0 / 0 | 0 / 0 | 0 / 0 | 0.99715 |

The repeatable effect remains **+4 unrouted / −119 copper violations / zero mask change across 34 changed, completed outputs**. This is the third full measurement of that trade. Main/control have 719 completed pairs and all 719 SES outputs are identical. Control/candidate have 719 completed pairs: 685 identical and 34 changed. The identical outputs account for −98 reported mask violations; KiCad mask scoring varies on fixed geometry. Deadline effects explain the large apparent connection gain. Neither a 302-connection improvement nor a 5% speed improvement is established.

PCBench completed boards rise 709→716 versus main (710→716 versus control); fully connected boards remain 588. Median RSS stays 12.1MB, maximum rises 366.4→368.7MB versus main. Local completed/connected counts remain 10/8, median RSS 12.1→13.6MB and maximum 68.3→76.9MB. CPU totals versus main are 34,710.65→32,971.54s PCBench and 777.45→780.55s local.

Ordinary connectivity losses remain minimal_node +2 U/−2 copper, azalea +4/−45, and MPPT +2/0. Of six added copper violations versus main, three occur on documented input outliers (Brushless +1, OLED +2), and three on deadline-affected boards (BLDC +1, chess +2). Outliers do not explain the ordinary connectivity losses. No board is excluded.

The [generated benchmark summary](routing-quality-artifacts/pad-local-clearance/post-main/pr-summary.md) reports a **failed regression gate with 13 routing-quality losses**. This remains an explicit accuracy/connectivity trade for review. [Main comparison](routing-quality-artifacts/pad-local-clearance/post-main/vs-main.json), [same-binary comparison](routing-quality-artifacts/pad-local-clearance/post-main/vs-control.json), and [output identity audit](routing-quality-artifacts/pad-local-clearance/post-main/identities.json) are retained. The integrated source passed a clean full workspace run: **2,586 passed, 77 ignored, zero failed**, including doctests; browser tests: **49 passed**. Production source hashes match the frozen candidate. Java recordings are unchanged.

A separate metadata diagnostic corrected three duplicate-reference names on 96boards Sensors by matching original and stripped footprint UUIDs and positions. With exactly the same binary and rule values, copper violations fell 13→0, unrouted stayed zero, vias 113→105 and wirelength 3747.95→3834.56mm. CPU was 153.67→153.08s, peak RSS 57.6→54.7MB. Both referees succeeded. This one-board pilot is not part of the full results above and is not yet a validated corpus-wide generator change. It demonstrates that a source-metadata census does not prove every rule reached the imported board.

## Earlier same-binary full comparison

`quality-epyc-pad-copper-matched-01`: 751 boards each for main e9d10c2, control, and candidate; all 2,253 KiCad referees successful. Control and candidate use the same frozen executable, with old/new metadata respectively. Ten passes, 300-second cap, one routing thread, 192 concurrent jobs. No Java scoring. Results are copied locally and to workbench.

The 719 pairs completed by both candidates have **four more unrouted connections and 119 fewer copper violations**. Of these, 685 SES outputs are byte-identical: all −7 reported mask difference occurs on identical outputs. The 34 changed completed outputs have +4 unrouted/−119 copper/zero mask difference. This is an accuracy/connectivity trade, not a demonstrated connection improvement.

| Group vs control | Unrouted delta / better / worse | Copper delta / gainers | Reported mask delta / gainers | CPU ratio |
|---|---:|---:|---:|---:|
| PCBench 740 | −459 / 17 / 3 | −118 / 4 | +39 / 10 | 0.9498 |
| Local 11 | 0 / 0 / 0 | 0 / 0 | 0 / 0 | 0.9938 |

The 32 deadline-affected pairs contribute −463 unrouted/+1 copper/+46 mask. Thus the headline −459 connections and CPU ratio are not reliable causal improvement claims. The first full run independently gave +4 unrouted/−119 copper on completed pairs, with a different control build; the repeat removes that build difference.

| Group | Completed | Connected | Median RSS MB | Maximum RSS MB |
|---|---:|---:|---:|---:|
| PCBench | 710→716 | 588→589 | 12.1→12.1 | 367.9→374.3 |
| Local | 10→10 | 8→8 | 13.6→13.6 | 66.0→79.1 |

## Regressions and outliers

Three ordinary boards lose connections: minimal_node +2 unrouted/−2 copper, azalea +4/−45, and MPPT-2420-HPX +2/0. MPPT's added 700µm fiducial clearances are real designer rules; its original is fully connected with zero routing DRC. None receives an outlier exemption.

Of five added copper violations in the repeat, three involve evidenced input outliers: Brushless_ESC +1 short against omitted B.Cu text, and OLED +2 collisions against a circular cutout absent from the rectangular DSN outline. OLED's original has 15 malformed-outline errors. These account for 60% of added copper violations in this repeat, versus 3/8 in the first full run. BLDC +1 and real-time-chess +1 are deadline-affected. See [outlier evidence](outlier-boards.md). Every board remains included; this concentration supports considering the DRC gain but does not explain ordinary connectivity losses.

BLDC needs particular follow-up: the pilot rose from 56.3MB to 375.8MB RSS and 165.1s to 243.2s CPU. Final geometry shrank, so retained output size does not explain the extra memory. Its nine→ten copper violations were all track-width undershoots (0.2498mm against 0.25mm), not clearance failures. A separate minimum-width neckdown retry is being measured; it is not included here.

## Input limitations and validation

The metadata census covers 751 boards; 78 have explicit values on 1,557 source pads. All annotated original pads match metadata records, but this does not prove DSN hydration: BLDC thermal-pad warnings Q2.5/Q4.5 occur on both candidates. Three boards have coincident same-name/same-position pads; metadata uses their maximum copper floor, which can be stricter than individual layer rules. Native import retains separate pads. The generator and census are retained with the artifacts. Native/browser import is automatic; DSN requires the existing `COPPERROUTE_PAD_CLEARANCE_JSON` sidecar.

Failing-first native, loader and browser tests cover gap-cap independence and override inheritance. Further tests cover rounding, higher-rule preservation and invalid values. All 49 browser tests pass. A clean explicit-manifest workspace run rebuilt this worktree and passed **2,584 tests, 77 ignored, zero failed**, including doctests. JVM recordings are unchanged. Frozen production hashes match the reviewed Rust sources.

The [generated benchmark summary](routing-quality-artifacts/pad-local-clearance/pr-summary.md) compares with the earlier main e9d10c2 and includes PR25 and dependencies. PCBench: −1236 unrouted/−166 copper/−4215 reported mask, 141 connectivity improvements/38 regressions, 24 copper gainers, CPU ratio 0.8781. Local: −14 unrouted/−2 copper/zero mask, three connectivity improvements/no regressions, one copper gainer, CPU ratio 0.9187. These gains are not all caused by pad clearance. The aggregate gate fails and one repetition does not establish speed. [Incremental results](routing-quality-artifacts/pad-local-clearance/vs-control.json), [main results](routing-quality-artifacts/pad-local-clearance/vs-main.json), and [completed-output hashes](routing-quality-artifacts/pad-local-clearance/completed-identities.json) are retained.
