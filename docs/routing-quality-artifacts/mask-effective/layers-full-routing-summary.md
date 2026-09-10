# Routing validation

KiCad only. Copper and mask findings are reported separately. A completed run may still have unrouted connections. Both-completed comparisons exclude external timeouts and require internal COMPLETED state on both sides. RSS is each board's process peak, not aggregate host memory. Mask reports at 199 findings may be capped. Single runs do not establish a speed improvement.

## all

| Candidate | Boards | Unrouted | Copper | Mask | Fully connected | Partial | Completed | Deadline | CPU s | Max RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| main | 751 | 4585 | 874 | 13906 | 602 | 149 | 722 | 29 | 33980.74 | 295.2 |
| mask-layers | 751 | 5280 | 886 | 13832 | 600 | 151 | 689 | 62 | 34392.29 | 295.0 |

| Baseline → candidate | Subset | Boards | U improved / worse | ΔU | Copper gainers / Δ | Mask gainers / Δ | Uncapped mask gainers / Δ (boards) | CPU ratio | Identical SES |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| main → mask-layers | all | 751 | 1 / 23 | +695 | 3 / +12 | 7 / -74 | 0 / -26 (724) | 1.0121 | 697 |
| main → mask-layers | both completed | 689 | 0 / 0 | +0 | 0 / +0 | 2 / -60 | 0 / +0 (676) | 1.1392 | 689 |

## pcbench

| Candidate | Boards | Unrouted | Copper | Mask | Fully connected | Partial | Completed | Deadline | CPU s | Max RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| main | 740 | 4355 | 816 | 13906 | 595 | 145 | 712 | 28 | 33256.06 | 295.2 |
| mask-layers | 740 | 5050 | 828 | 13832 | 593 | 147 | 679 | 61 | 33673.34 | 295.0 |

| Baseline → candidate | Subset | Boards | U improved / worse | ΔU | Copper gainers / Δ | Mask gainers / Δ | Uncapped mask gainers / Δ (boards) | CPU ratio | Identical SES |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| main → mask-layers | all | 740 | 1 / 23 | +695 | 3 / +12 | 7 / -74 | 0 / -26 (713) | 1.0125 | 687 |
| main → mask-layers | both completed | 679 | 0 / 0 | +0 | 0 / +0 | 2 / -60 | 0 / +0 (666) | 1.1427 | 679 |

## local-kicad

| Candidate | Boards | Unrouted | Copper | Mask | Fully connected | Partial | Completed | Deadline | CPU s | Max RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| main | 11 | 230 | 58 | 0 | 7 | 4 | 10 | 1 | 724.68 | 132.9 |
| mask-layers | 11 | 230 | 58 | 0 | 7 | 4 | 10 | 1 | 718.95 | 114.4 |

| Baseline → candidate | Subset | Boards | U improved / worse | ΔU | Copper gainers / Δ | Mask gainers / Δ | Uncapped mask gainers / Δ (boards) | CPU ratio | Identical SES |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| main → mask-layers | all | 11 | 0 / 0 | +0 | 0 / +0 | 0 / +0 | 0 / +0 (11) | 0.9921 | 10 |
| main → mask-layers | both completed | 10 | 0 / 0 | +0 | 0 / +0 | 0 / +0 | 0 / +0 (10) | 0.9860 | 10 |
