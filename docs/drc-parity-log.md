# KiCad DRC parity work

Branch `fix/kicad-drc-parity`, based on origin/main 736d02d. Work is separate from the frozen structural evaluation running on the replacement server. User prioritizes agreeing on routing-related defects and pass/fail, not report ordering or runtime. Target referee version is KiCad 10.0.6. Local 10.0.3 is used as an additional cross-check.

## Confirmed missing unnetted-pad mask check

The existing solder-mask validation report records three missed unnetted-pad contacts on Own-Mailbox. A minimal reproduction derived from the existing pad-track oracle removes the pad's net assignment, keeping the geometry and rules. Both KiCad versions report two front mask bridges and no copper-clearance violations. The Rust regression failed with zero bridges before removing the unnetted-pad early skip. After the change, the full copper-drc crate test suite passes. No existing oracle recordings were replaced; a new 10.0.6 oracle was added for this new case. No commit or merge yet.

## Remaining parity work

* Quantify individual findings against KiCad on the same geometry, project, filled zones and candidate output; do not infer recall from raw report totals.
* Extend mask coverage for other item pairs and artwork, with independent KiCad fixtures and correct bridge exceptions. Unnetted pad/track cases and basic pad-pair coverage now have independent oracles.
* Thermal-relief/starved-thermal coverage and the zone-fill geometry required to reproduce it.
* Rule resolution: local overrides, .kicad_dru rules, severities, exclusions and inheritance.
* Connectivity/dangling findings must be included when choosing candidate routes; `get_all_violations()` alone excludes dangling findings added during full report generation.
* Audit supported copper/hole/edge checks on real mismatches, including import loss and geometry approximations.
* Run workspace and web tests plus a full matched benchmark under CONTRIBUTING.md before landing behaviour changes. Keep unresolved mismatches explicit; no claim of complete parity.

The additional between-pass grid/protection experiments are deferred until DRC improvements land, as requested.

## Additional mask oracles

Two more KiCad 10.0.6 fixtures confirm that netless tracks must be checked against pads and that a pad/track pair with both nets unassigned is not a same-net exception. Both regression tests failed before removing the track-net skip; all seven mask tests then passed.

Pad-to-pad apertures: two 1 mm pads 1.3 mm apart, each with 0.2 mm mask expansion, have legal 0.3 mm copper clearance but overlapping mask apertures. KiCad reports one bridge; the new test initially reported zero internally. The check now includes pad copper, enlarges the candidate query for the other pad's mask expansion, computes rounded/circular core distances, and deduplicates by unordered pair and layer. Eight mask tests passed at this stage. More exception and geometry cases remain required.

Minimum mask web: two such pads 1.5 mm apart leave 0.1 mm mask between their openings. A 0.2 mm minimum should flag the pair. The first oracle mistakenly set only the historical project `rules.solder_mask_min_width`, which KiCad 10 did not apply. Inspection of the pinned 10.0.6 board design settings source showed that mask width moved back to the board during schema migration. After adding `(solder_mask_min_width 0.2)` to the PCB setup, KiCad reported one bridge and the Rust test failed for the expected missing check. `DrcConstraints` now carries the web minimum and the pad-pair check uses it; the regression sets that model field directly. Import/export propagation of the board setting is still outstanding. Do not interpret the geometry test as end-to-end import coverage.

Pinned upstream code inspected: KiCad/kicad-source-mirror tag 10.0.6, `pcbnew/drc/drc_test_provider_solder_mask.cpp` and `pcbnew/board_design_settings.cpp`. Source references were used to guide independent KiCad fixture measurements; original reports remain unchanged.

## Board setting propagation

Added optional `solderMaskMinWidth` to the native JSON model, stored independently on BoardRules and fed into resolved DRC constraints. The writer preserves it in board units; absent metadata remains absent. The browser adapter reads `(solder_mask_min_width ...)` from PCB setup. New Rust import/export test failed with None vs Some(2000); the corrected adapter test failed with undefined vs 0.2 before implementation. After implementation, the copper-drc and copper-dsn suites pass and all 50 web tests pass. The first adapter fixture had no outline and was rejected before reaching the assertion; it was replaced with the existing valid adapter fixture before testing the metadata failure.

Remaining pad-pair exceptions (including net ties), via mask apertures, mask artwork, thermal/zone checks, and full native corpus finding matching remain unfinished. The current geometry fixtures do not establish full KiCad parity.

## Pad-pair exceptions

Three KiCad 10.0.6 controls confirm no mask finding for same-net pads, diagonal corner clearance, and explicitly allowed bridges within a footprint. The combined Rust test passed the first two but failed for board-wide footprint permission (one false positive vs zero). Added `allowSolderMaskBridgesInFootprints` across the browser/native JSON model, BoardRules and JSON export; the pair check applies the board permission only to pads in the same nonzero component. Both Rust DRC/DSN suites pass and 51 web tests pass. The workspace suite completed successfully for the preceding mask-web import version; it still needs rerunning for the latest permission change before any commit.

## Frozen first mask candidate

The board-wide permission has an additional positive control: it does not excuse a foreign routing track. KiCad 10.0.6 still reports one mask bridge; the Rust per-footprint permission test now also carries the board-wide permission. Latest workspace test run is recorded in `/tmp/quality-drc-parity/workspace-v2.log`.

A separate server build/run is queued behind structural-full-01: `drc-parity-mask-01`, 751 KiCad-refereed boards, 112 one-thread workers, 10 passes and 300 seconds, compared with the fresh pristine-main baseline. This DSN run validates routing compatibility and CPU cost, not mask recall: DSN omits mask metadata. The independent KiCad native geometry/import oracles establish the added checks, and broader native finding comparisons are still required.

Frozen source archive is on workbench at `/home/em/copperroute-epyc-results/web-structural-routing/server/evaluated-mask-source.tgz`. The copied mask checker, reader and BoardRules SHA256 hashes match the laptop exactly. Workbench backup process was restarted as PID 760683 and now continues through the DRC candidate evaluation. No commits or merges yet.

Latest frozen mask candidate workspace validation completed: 2615 passed, zero failed, 77 ignored (`workspace-v2.log`). Full routing compatibility evaluation remains queued behind the last structural jobs.

## Real connectivity mismatch from structural evaluation

The completed structural evaluation identified `pcbench-esper_programmer`: grid repair reduces internal incompletes 2→1 while KiCad after refill worsens 12→13. New ground/3V3 zone connectivity reports appear. The original human-routed board, checked with the same KiCad 10.0.6/project/refill, has zero unconnected items and zero copper-routing errors. This is not established as an outlier. It is a concrete board for testing the missing final-pour/connectivity parity; exact path/zone changes still need diagnosis.

The first DRC mask candidate build finished and `drc-parity-mask-01` is now running. Large results remain backed up to workbench.

## Non-plated holes: false positive and missing copper

Independent KiCad 10.0.6 fixtures distinguish a 1 mm circular NPTH pad with a 1 mm circular drill (zero mask bridges) from a 1 mm square NPTH pad with that drill (two mask bridges at the remaining copper). The Rust circle test first failed with two false positives; the mask checker now exempts aperture owners with no copper, matching its existing filter for the other item. The square control then failed with zero vs two: the native importer used maximum width rather than the enclosing-circle diameter to determine whether a circular drill consumes all copper. Corrected that calculation for rectangular/rounded rectangular pads while retaining circle and oval geometry. Both failure logs are in /tmp/quality-drc-parity, and the fixtures preserve original KiCad findings rather than suppressing unrelated hole-clearance errors.

These changes postdate the frozen drc-parity-mask-01 source. Its full run must not be described as validation of this revised native importer/checker; final source validation remains required.

## Completed first compatibility evaluation

`drc-parity-mask-01` scored all 751 KiCad boards, zero failed referees, against main 736d02d from `structural-full-01`. KiCad 10.0.6, 112 single-thread workers, 10 passes, 300-second limit. Workbench has the full results and exports; its backup completed on 2026-09-10 at 16:07:08 UTC. Compact comparison: `drc-parity-mask-01-comparison.json`.

| Scope | Boards | Unrouted improved/regressed | Unrouted delta | Copper-gaining boards / delta | Mask-gaining boards / delta | CPU ratio |
|---|---:|---:|---:|---:|---:|---:|
| all-all | 751 | 16/4 | -272 | 1 / -10 | 2 / 15 | 0.9584 |
| all-completed | 697 | 0/0 | 0 | 0 / 0 | 0 / 0 | 0.9541 |
| pcbench-all | 740 | 16/4 | -272 | 1 / -10 | 2 / 15 | 0.9574 |
| pcbench-completed | 688 | 0/0 | 0 | 0 / 0 | 0 / 0 | 0.9535 |
| freerouting-kicad-all | 11 | 0/0 | 0 | 0 / 0 | 0 / 0 | 0.9996 |
| freerouting-kicad-completed | 9 | 0/0 | 0 | 0 / 0 | 0 / 0 | 0.9990 |

Main/candidate totals: 5112/4840 unrouted, 871/861 ordinary copper errors, 13731/13746 mask errors, 600/601 fully connected, 54/43 router deadlines, 46992.00/45034.96 CPU seconds, maximum per-board RSS 291.5/292.4 MiB. All quality changes involve a timeout on at least one side. The 697 pairs finishing on both sides have identical quality; do not attribute timeout-driven differences to mask accuracy or claim a speed gain from this single run. Ordinary benchmark violations omit mask bridges, hence separate columns. No Java referee rows were run. This validates only the frozen v1 source, before NPTH fixes.

## Native-board differential survey setup

The revised NPTH source is frozen separately at server `/root/copperroute-drc-native`, with its own `/root/drc-native-build` target. `native-mask-parity-01` compares main and this candidate on identical native JSON imported from the 751 routed KiCad outputs of structural-full-01. The current browser adapter preserves routing and pad mask metadata; project net classes and DRC rules are applied to both. Each report is compared with that board's existing KiCad 10.0.6 referee report. A 112-worker driver records unsupported imports, crashes and timeouts separately; a nonzero CLI status with a valid DRC report is a finding, not an import failure.

Important scope limits: the adapter excludes zone fills from the internal model, and warns about unsupported rules/geometry. Counts alone do not demonstrate pairwise finding agreement. This survey identifies concrete mismatches for investigation; it cannot prove full DRC parity. Native source, scripts, inputs and reports stay on the server and are backed up directly to workbench by backup-native.sh; only compact summaries should reach the laptop.

## Native survey result and Feather identity regression

The first native survey finished all 751 attempted imports: 549 produced reports from both Rust versions; 202 were unsupported imports or failures. KiCad mask counts total 10131 on those paired boards, main 6458, candidate 13722. Those sums are not comparable defect totals because 20 reports reached KiCad's 199-finding cap. Among the 529 reports below that cap, mask-count distance improved on 119 boards and worsened on one. Counts alone do not establish finding correspondence.

The single worsened board is pcbench-Feather-ICE40-PCB_feather_ice40: KiCad/main/candidate mask counts 0/0/6. All six candidate findings concern the four unnetted rectangular pieces of U5 pad 0. The browser adapter places each physical pad as its own component (necessary for individual pad angles) and assigned synthetic pad numbers, losing logical pad identity. KiCad's pinned 10.0.6 `PAD::SameLogicalPadAs` compares parent footprint and equal nonempty pad number. New independent KiCad fixtures confirm zero bridges for two same-number unnetted pads and one bridge for two unnumbered pads.

The native JSON adapter now preserves sourceFootprint and sourcePadNumber. Pin preserves both through copy/equality, and mask checks use that identity for logical-pad and board-wide same-footprint exceptions. Empty pad numbers do not excuse a pair. Rust and browser tests failed before the fix; all 13 mask tests and 52 web tests pass afterward. Controls cover different pad numbers, different footprints, empty numbers, and board permission. A second 751-board native survey is rebuilding on the server as native-mask-parity-02. The prior workspace validation passed 2616 tests, zero failures, 77 ignored; the identity change requires a fresh workspace run.

## Logical identity fix: second native survey

native-mask-parity-02 again attempted 751 boards, with the same 549 paired reports and 202 unsupported/failed imports. Among 529 pairs below KiCad's 199-mask-finding cap, mask-count distance improves on 119 boards and worsens on zero. Feather's six false warnings are removed: KiCad/main/candidate now 0/0/0. This establishes the measured regression is fixed, but does not establish full pairwise finding agreement or complete KiCad parity. Source/scripts and binary hash are saved beneath results/native-mask-parity-02 and backed up to workbench. Compact summaries are retained locally.

The final revised source is now running the full DSN compatibility benchmark as drc-parity-mask-02: 751 KiCad boards, 112 workers, ten passes, 300 seconds, compared with the unchanged latest origin/main 736d02d. Native build target is frozen until that run completes. All large artifacts are backed up directly by backup-mask-final.sh. Workspace-v4 is running for the logical identity change. No commit or PR yet.

## Matching pad-pair findings, beyond aggregate counts

A second diagnostic matches the unordered pair of pad positions within 0.001 mm on the 529 uncapped native boards (normalizing the native report Y axis and retaining pair multiplicities). It includes KiCad's Pad, PTH pad and NPTH pad descriptions. KiCad reports 4509 pad-pair mask findings; main matches none; the candidate matches 4413. There are 36 unmatched candidate pairs and 96 unmatched KiCad pairs. This is spatial matching, not UUID/layer-perfect correspondence; offsets, duplicate positions and layer reporting still require inspection. Initial strict rounded-coordinate matching and a classifier omitting PTH pads understated matches; the saved script corrects both. Script/results live under results/native-mask-parity-02 and are backed up to workbench.

Ten unmatched candidate pairs on pcbench-zx-sizif-xxs_sizif-xxs involve 45-degree rectangular resistor-array pads, 0.8 x 0.4 mm, spaced 0.8 mm with 0.2 mm mask expansion per pad. Their apertures nominally just touch. Native coordinates are rounded to the router grid, so this is a concrete precision-boundary case for follow-up. It is not yet proven whether all ten arise solely from quantization; no blanket epsilon patch has been applied. The other unmatched candidate pairs are on kitspace_sensor (22) and rs485-moist-sensor_adapter-por (4), with equal numbers of unmatched KiCad pairs and potentially different reported anchor positions. Count improvement must not be represented as zero false positives.

Latest identity-fix workspace validation completed: 2617 passed, 0 failed, 77 ignored across 176 suites (workspace-v4.log).

## Resolving reporting mismatches

Sensor's 22 unmatched pad pairs have a 0.15 mm difference between the pad anchor and its offset copper shape. Native JSON explicitly retains these shapeOffset values. KiCad reports the pad anchor; the current Rust report uses its copper center. This is a reporting-coordinate mismatch requiring follow-up rather than evidence that the underlying pair checks are wrong.

RS485 adapter's raw corpus board contains ten repeated UUID values (multiplicities 7,7,5,5,2,2,2,2,2,2). A diagnostic copy assigning unique UUIDs changes no geometry and still produces 22 KiCad mask findings; all eight pad-pair findings then match the candidate, versus four previously. Added this to the outlier register as a report-identity issue only. The original corpus and headline references remain unchanged. Scripts and reports are backed up under native-mask-parity-02/duplicate-uuid-audit.

## Final full routing evaluation

Completed drc-parity-mask-02: 751/751 KiCad boards scored, zero referee failures. Full totals main/candidate: 5112/4838 unrouted, 871/857 ordinary copper errors, 13731/13747 mask errors, 600/601 fully connected, 54/43 router deadlines, 46992.00/45106.75 CPU seconds, maximum per-board RSS 291.5/293.2 MiB. On 696 pairs completing on both sides, unrouted/copper/mask counts are unchanged. Latest origin/main remains 736d02d. Nine changed implementation files have identical local/evaluated SHA256 hashes. Full report and origin splits are in drc-parity-evaluation.md.

The benchmark gate fails on four unrouted losses and two small score losses; all six hit the deadline in both runs. An initial one-pass control retained the optimizer because --max-passes affects only routing. Stopped that incomplete run deliberately and retained its artifacts with CONTROL-ABORTED.txt. Replacement drc-mask-one-pass-no-opt explicitly disables router.optimizer.enabled for both candidates, uses one routing pass and a 1200-second cap. It is a targeted control, not a substitute for the full evaluation.
