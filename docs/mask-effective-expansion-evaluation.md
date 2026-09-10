# KiCad solder-mask accuracy evaluation

Candidate `mask-effective-v1`, based on main `7ecbeac250ff4b78877610dad3f71c4e96ba4c08`. The full routing evaluation and completion controls are complete. This is proposed as a checker-accuracy correction; the 300-second routing gate still fails and its trade-offs are disclosed below.

The cumulative change handles pad mask openings on the opposite copper side, rectangular openings that touch with zero expansion, hole-only pads acting as copper-side participants, and effective expansion on pads without declared openings. Existing net, logical-pad and footprint exceptions remain. Effective expansion is stored independently from declared openings, including through import, pin copying and equality; it does not create an opening or routing obstacle.

Independent KiCad fixtures reproduce each case. For example, two 0.8 mm circular pads 1.1 mm apart produce one mask finding when a source pad without an opening has effective expansion 0.2 mm, and none with zero expansion. Raising minimum mask web to 0.4 mm still produces none in the zero-expansion source case, confirming that copper-to-mask clearance and mask web are distinct rules. The browser and Rust regression tests were witnessed failing before implementation.

## Checker accuracy

The native survey reimported 549 supported saved routed boards, including 529 with uncapped KiCad mask reports. It carried forward 202 original import exclusions without retrying them. See [coverage and excluded boards](native-drc-coverage.md). Supported reimports were verified identical to the prior inputs after removing the new effective-expansion field and normalizing only the design-name path. The new binary on old inputs retained the preceding candidate's error-type counts.

Against main, seven uncapped boards moved closer to KiCad's mask count and none moved farther; other DRC-type counts were unchanged. Matched KiCad pad-pair findings increased from 4,413 to 4,483 out of 4,509; unmatched candidate pairs stayed at 36 and missing reference pairs fell from 96 to 26. Matching uses positions within 1 µm and retains multiplicity, so it is not UUID/layer-perfect proof. Effective expansion alone adds five confirmed pairs on Baofeng and Xmas Ornament relative to the preceding cumulative candidate.

Isolated KiCad controls also support the added findings on Dropbot and Mechaduino, whose whole-board reports reach the cap. See [capped-report audits](mask-capped-report-audits.md). Remaining discrepancies include offset anchors, duplicate UUIDs and rotated integer geometry; see [precision investigation](mask-precision-investigation.md). The historical reference reports use KiCad 10.0.6, while independent fixtures used 10.0.3/10.0.4. This is evidence of improved coverage, not complete version-independent parity.

## Full routing comparison

All 751 boards have valid KiCad scores: 740 PCBench and 11 local fixtures. Settings: one router thread, 12 workers, 10 passes, 300-second wall limit, one seed. Referee: KiCad 10.0.4. The unchanged benchmark gate flags **seven routing-quality losses**. Full [split report](routing-quality-artifacts/mask-effective/full-routing-summary.md), [per-board data](routing-quality-artifacts/mask-effective/full-routing-summary.json), and [generated PR report](routing-quality-artifacts/mask-effective/full-pr-summary.md) are retained.

| Metric | Main | Candidate |
|---|---:|---:|
| Unrouted connections | 4,585 | 4,611 |
| Copper violations | 874 | 873 |
| Mask findings | 13,906 | 13,914 |
| Fully connected boards | 602 | 602 |
| Partially connected boards | 149 | 149 |
| Completed routing jobs | 722 | 724 |
| Internal or external deadlines | 29 | 27 |
| CPU seconds | 33,980.74 | 32,518.72 |
| Maximum per-process RSS MiB | 295.2 | 295.1 |

| Group | Boards | Unrouted better / worse | Δ unrouted | Copper gainers / Δ | Mask gainers / Δ | CPU ratio |
|---|---:|---:|---:|---:|---:|---:|
| All | 751 | 9 / 4 | +26 | 1 / -1 | 14 / +8 | 0.9570 |
| PCBench | 740 | 9 / 4 | +26 | 1 / -1 | 14 / +8 | 0.9556 |
| Local KiCad | 11 | 0 / 0 | 0 | 0 / 0 | 0 / 0 | 1.0210 |

All 722 pairs where both routers completed have byte-identical SES and equal unrouted/copper counts. All 704 such pairs below the mask-report cap have identical mask counts. Across all 724 uncapped pairs, there is one mask gainer and net +9 findings. CPU ratios are single-run observations, not evidence of a speed improvement.

The four unrouted regressions are Blitz_.C68 (+97), Blitz_copy (+1), karabas-nano-revB (+3), and karabas-nano-revG (+12). Each pair timed out with similar CPU availability. nonSNES improves 188→161 unrouted but gains one copper violation and 17 mask findings (11→28); both runs timed out. These differences are not dismissed as outliers or attributed solely to contention. The added nonSNES copper violation crosses a real footprint copper rectangle, exposing a separate DSN obstacle-representation and native checker-coverage gap. Its exact clear/crossing controls are archived on workbench.

Earlier layer-only and layer-plus-overlap runs had substantially more deadlines while sharing the host with an independent 12-worker evaluation. Their completed-pair routes were also identical. Those observations do not explain away the final candidate's regressions, which received similar CPU time to main. Detailed intermediate numbers remain in the full split report and running log.

`mask-deadline-control-01` completed all 99 jobs on 33 regression boards (31 PCBench, two local), comparing main, layer-only and final cumulative binaries. One pass, optimization disabled, one thread, 12 workers, 1200-second cap. All 66 candidate/main comparisons have byte-identical SES and equal unrouted/copper counts, including both Blitz boards, karabas-nano-revB/revG and nonSNES. All 23 uncapped mask comparisons per candidate also agree. Capped mask counts vary on identical routes, as in the earlier referee audit.

Main/final control totals: 3,774 unrouted each, 195 copper each, CPU 4,176.49/3,729.66 seconds, peak RSS 108.2/107.6 MiB. The CPU ratio is 0.8930 (PCBench 0.8877, local 1.0749), with no speed claim. Raw mask totals 3,196/3,193 include capped variability. Full [control report](routing-quality-artifacts/mask-effective/deadline-control-summary.md) and [all cells](routing-quality-artifacts/mask-effective/deadline-control-summary.json) are retained.

These controls establish first-pass equivalence; they do not replace the full 10-pass comparison or erase its deadline-dependent regressions. Together with the 722 identical completed full-run pairs and independently confirmed checker-accuracy gains, they support proposing the mask change separately from routing-algorithm improvements.

## Other validation and limits

Full workspace: 2,622 passed, zero failed, 77 ignored. Web: 53 passed. All 48 changed production/test files match the evaluated workbench snapshot. Full logs are archived under `copperroute-mask-results/provenance/effective-v1/`.

Native Nano/Feather controls completed without deadlines using newly imported effective metadata on both sides. Nano remains 0 unrouted/4 inherited hole-clearance findings; Feather remains 7 unrouted/35 routing findings plus 8 courtyard findings. Each board's native output is byte-identical. CPU seconds were 10.64→10.68 and 245.51→246.36; peak RSS MiB 14.86→15.15 and 76.85→78.48. Compatibility evidence only.

Known GTK import failures in earlier runs were rescored only after preserving their original evidence and verifying the exact saved SES and routing measurements were unchanged. All full-run cells are now valid. Exact-file KiCad checks showed that high/capped mask counts can vary, and UUID/top-level ordering can change short-versus-clearance classification without changing stored geometry. These findings and recovery records are archived under `provenance/referee-repeat-audit` and `provenance/automatic-referee-recovery`.

This change does not establish full KiCad parity: custom rules, mask-only graphics, zone-fill behavior, arbitrary padstacks and unsupported imports remain limitations. The independent circular-copper and rounded-pad/track fixes are not part of this candidate. No global routing-grid or tolerance change is included.
