# Routing validation

KiCad only. Copper and mask findings are reported separately. A completed run may still have unrouted connections. Both-completed comparisons exclude external timeouts and require internal COMPLETED state on both sides. RSS is each board's process peak, not aggregate host memory. Mask reports at 199 findings may be capped. Single runs do not establish a speed improvement.

## all

| Candidate | Boards | Unrouted | Copper | Mask | Fully connected | Partial | Completed | Deadline | CPU s | Max RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| main | 751 | 4585 | 874 | 13906 | 602 | 149 | 722 | 29 | 33980.74 | 295.2 |
| mask-layers | 751 | 5280 | 886 | 13832 | 600 | 151 | 689 | 62 | 34392.29 | 295.0 |
| mask-both | 751 | 5254 | 884 | 13864 | 600 | 151 | 683 | 68 | 34848.73 | 295.0 |

| Baseline → candidate | Subset | Boards | U improved / worse | ΔU | Copper gainers / Δ | Mask gainers / Δ | Uncapped mask gainers / Δ (boards) | CPU ratio | Identical SES |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| main → mask-layers | all | 751 | 1 / 23 | +695 | 3 / +12 | 7 / -74 | 0 / -26 (724) | 1.0121 | 697 |
| main → mask-layers | both completed | 689 | 0 / 0 | +0 | 0 / +0 | 2 / -60 | 0 / +0 (676) | 1.1392 | 689 |
| main → mask-both | all | 751 | 2 / 28 | +669 | 6 / +10 | 10 / -42 | 2 / -33 (724) | 1.0255 | 690 |
| main → mask-both | both completed | 682 | 0 / 0 | +0 | 0 / +0 | 5 / +14 | 0 / +0 (669) | 1.1978 | 682 |
| mask-layers → mask-both | all | 751 | 12 / 10 | -26 | 4 / -2 | 12 / +32 | 2 / -7 (724) | 1.0133 | 691 |
| mask-layers → mask-both | both completed | 673 | 0 / 0 | +0 | 0 / +0 | 5 / +62 | 0 / +0 (662) | 1.0551 | 673 |

## pcbench

| Candidate | Boards | Unrouted | Copper | Mask | Fully connected | Partial | Completed | Deadline | CPU s | Max RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| main | 740 | 4355 | 816 | 13906 | 595 | 145 | 712 | 28 | 33256.06 | 295.2 |
| mask-layers | 740 | 5050 | 828 | 13832 | 593 | 147 | 679 | 61 | 33673.34 | 295.0 |
| mask-both | 740 | 5008 | 826 | 13864 | 593 | 147 | 675 | 65 | 34147.28 | 295.0 |

| Baseline → candidate | Subset | Boards | U improved / worse | ΔU | Copper gainers / Δ | Mask gainers / Δ | Uncapped mask gainers / Δ (boards) | CPU ratio | Identical SES |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| main → mask-layers | all | 740 | 1 / 23 | +695 | 3 / +12 | 7 / -74 | 0 / -26 (713) | 1.0125 | 687 |
| main → mask-layers | both completed | 679 | 0 / 0 | +0 | 0 / +0 | 2 / -60 | 0 / +0 (666) | 1.1427 | 679 |
| main → mask-both | all | 740 | 2 / 26 | +653 | 6 / +10 | 10 / -42 | 2 / -33 (713) | 1.0268 | 682 |
| main → mask-both | both completed | 674 | 0 / 0 | +0 | 0 / +0 | 5 / +14 | 0 / +0 (661) | 1.1973 | 674 |
| mask-layers → mask-both | all | 740 | 12 / 8 | -42 | 4 / -2 | 12 / +32 | 2 / -7 (713) | 1.0141 | 683 |
| mask-layers → mask-both | both completed | 665 | 0 / 0 | +0 | 0 / +0 | 5 / +62 | 0 / +0 (654) | 1.0540 | 665 |

## local-kicad

| Candidate | Boards | Unrouted | Copper | Mask | Fully connected | Partial | Completed | Deadline | CPU s | Max RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| main | 11 | 230 | 58 | 0 | 7 | 4 | 10 | 1 | 724.68 | 132.9 |
| mask-layers | 11 | 230 | 58 | 0 | 7 | 4 | 10 | 1 | 718.95 | 114.4 |
| mask-both | 11 | 246 | 58 | 0 | 7 | 4 | 8 | 3 | 701.45 | 66.1 |

| Baseline → candidate | Subset | Boards | U improved / worse | ΔU | Copper gainers / Δ | Mask gainers / Δ | Uncapped mask gainers / Δ (boards) | CPU ratio | Identical SES |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| main → mask-layers | all | 11 | 0 / 0 | +0 | 0 / +0 | 0 / +0 | 0 / +0 (11) | 0.9921 | 10 |
| main → mask-layers | both completed | 10 | 0 / 0 | +0 | 0 / +0 | 0 / +0 | 0 / +0 (10) | 0.9860 | 10 |
| main → mask-both | all | 11 | 0 / 2 | +16 | 0 / +0 | 0 / +0 | 0 / +0 (11) | 0.9679 | 8 |
| main → mask-both | both completed | 8 | 0 / 0 | +0 | 0 / +0 | 0 / +0 | 0 / +0 (8) | 1.2974 | 8 |
| mask-layers → mask-both | all | 11 | 0 / 2 | +16 | 0 / +0 | 0 / +0 | 0 / +0 (11) | 0.9757 | 8 |
| mask-layers → mask-both | both completed | 8 | 0 / 0 | +0 | 0 / +0 | 0 / +0 | 0 / +0 (8) | 1.2944 | 8 |
