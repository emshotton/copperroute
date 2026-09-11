# Rounded copper clearance evaluation

This corrects internal KiCad-style DRC distances around circular copper and rounded rectangular pads. Native copper-violation counts move closer to KiCad on 70 boards, with none farther away. The routing comparison and fixed-pass controls support landing this as an accuracy correction, not as a routing-algorithm or speed improvement. The full-run benchmark gate still reports six routing-quality losses; those observations are retained below.

## Mechanism and accuracy

Enclosing tile shapes can report clearance errors outside the actual rounded boundary. Circular copper now shares the existing hole-to-copper distance calculation. Rounded-pad/track pairs measure the inset pad core against the track centreline and subtract both radii. Existing net, layer, logical-pad and rule guards remain. The mask segment-distance helper moves unchanged for reuse.

On isolated Sizif geometry, valid pad/via and pad/track gaps of 0.205606 mm and 0.219481 mm exceed the 0.2 mm rule: KiCad reports no violation and the old checker reports one. Closer controls at 0.158604 mm and 0.148770 mm remain violations in both. Tests failed for these false positives before production edits. The combined candidate passes all 2,627 workspace tests, with 77 ignored; no JVM recordings changed.

The combined native survey attempted 751 boards, retaining 202 inherited import exclusions. All 549 supported copper reports are uncapped: 70 boards are closer to historical KiCad counts, none farther, and non-copper error counts are unchanged. On the saved Sizif input, copper findings fall from 70 to zero, matching KiCad; mask findings remain 175. Count agreement is not item-level parity. Historical native reference reports use KiCad 10.0.6; new routing refereeing uses KiCad 10.0.4.

## Full routing comparison

Baseline is main 5c4a5c94f30392e83a6293aaeba9c1b9bc25bdcc. All 751 boards per side are KiCad-judged; no Java referee is used. Run `rounded-copper-full-01` uses one thread, 12 jobs, 10 passes, and a 300-second deadline. CPU is process CPU, and RSS is each board's process peak.

| Metric | Main | Rounded copper |
|---|---:|---:|
| Unrouted connections | 4,589 | 4,553 |
| Copper/routing DRC findings | 870 | 872 |
| Solder-mask findings | 13,941 | 13,942 |
| Fully connected / partial boards | 602 / 149 | 601 / 150 |
| Completed / deadline runs | 722 / 29 | 726 / 25 |
| CPU seconds | 33,970.58 | 32,567.98 |
| Maximum peak RSS MiB | 295.2 | 295.1 |

| Group | Boards | U better / worse | Net U | Copper gainers / net | Mask gainers / net | CPU ratio |
|---|---:|---:|---:|---:|---:|---:|
| PCBench | 740 | 7 / 5 | -36 | 2 / +2 | 12 / +1 | 0.9596 |
| Local KiCad | 11 | 0 / 0 | 0 | 0 / 0 | 0 / 0 | 0.9186 |
| All | 751 | 7 / 5 | -36 | 2 / +2 | 12 / +1 | 0.9587 |

All 722 pairs completed on both sides produce byte-identical SES, with unchanged unrouted and copper counts. Their 704 uncapped mask pairs also have unchanged mask counts. Across the full run, 724 mask pairs are uncapped, with one gainer and net -1; differences on identical SES occur only at the reporting cap. The benchmark gate fails with six routing-quality losses. No speed improvement is claimed from one repetition; an equivalent-source baseline repeat itself varied by 4.46% in CPU.

## Regression investigation and matched-pass controls

Every unrouted/copper difference involves deadline cases. The five connectivity regressions are Blitz_.C68 (+7), bms-8s50-ic (+1), decelerator4030 (+17), karabas-nano-revC (+4), and dropbot-front-panel (+2, losing full connectivity). Copper gainers are decelerator4030 (10 to 11 clearance findings) and real-time-chess (70 to 72 edge-clearance findings). The uncapped mask gainer is kitspace_T32_ref (179 to 181). Both arms hit the deadline on each of these boards. They are not labelled outliers.

Original-board inspection confirms that the chess designer respects the circular cutouts. Near the inspected /s14 route and the 12.6 mm radius cutout centred at (215.000022, 85), the original minimum calculated gap is 0.85 mm; both timed-out routes reach about 0.2099 mm against a 0.5 mm rule. The original has zero routing DRC findings. Decelerator's original also has zero routing DRC findings, although its outline has two existing errors; those errors do not excuse the new clearance violation. UUID-independent comparison identifies the extra decelerator /A3 track-to-+5V-via finding at 0.1279 mm against 0.2 mm. The other changed /DRM26 finding moves between track segments without changing its 0.1790 mm gap.

`rounded-copper-deadline-control-01` reruns all 13 boards with changed connectivity, copper, or uncapped mask counts: one pass, optimizer disabled, 1,200-second limit, same binaries and explicit KiCad referee environment. All 26 cells complete without a deadline. All 13 SES pairs are byte-identical, with 2,225 unrouted and 82 copper findings on each side. The nine uncapped mask pairs are unchanged; one capped report differs by one. CPU ratio is 0.8689, also not a speed claim. These controls establish equality at matched routing work; they do not erase the 10-pass deadline trade-offs.

## Artifacts and limits

The [full generated benchmark report](routing-quality-artifacts/rounded-copper/full-pr-summary.md), [split routing report](routing-quality-artifacts/rounded-copper/full-routing-summary.md), and [matched-pass report](routing-quality-artifacts/rounded-copper/deadline-control-summary.md) include the comparison data. Raw reports, source archives, binary hashes and logs remain on workbench under `copperroute-mask-results/provenance/rounded-copper-v1`. The 22 evaluated source/test/fixture fingerprints are checked against this worktree. Large routed-board artifacts remain on workbench.

This does not establish complete KiCad parity. In particular, footprint copper graphics and internal cutout checking have separate coverage gaps; the former has an independent prototype. More accurate clearance counts must not be confused with proof that an otherwise unchecked route is valid.
