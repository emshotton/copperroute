# Routing validation

KiCad only. Copper and mask findings are reported separately. A completed run may still have unrouted connections. Both-completed comparisons exclude external timeouts and require internal COMPLETED state on both sides. RSS is each board's process peak, not aggregate host memory. Mask reports at 199 findings may be capped. Single runs do not establish a speed improvement.

## all

| Candidate | Boards | Unrouted | Copper | Mask | Fully connected | Partial | Completed | Deadline | CPU s | Max RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| main | 751 | 4589 | 870 | 13941 | 602 | 149 | 722 | 29 | 33970.58 | 295.2 |
| rounded-copper | 751 | 4553 | 872 | 13942 | 601 | 150 | 726 | 25 | 32567.98 | 295.1 |

| Baseline → candidate | Subset | Boards | U improved / worse | ΔU | Copper gainers / Δ | Mask gainers / Δ | Uncapped mask gainers / Δ (boards) | CPU ratio | Identical SES |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| main → rounded-copper | all | 751 | 7 / 5 | -36 | 2 / +2 | 12 / +1 | 1 / -1 (724) | 0.9587 | 731 |
| main → rounded-copper | both completed | 722 | 0 / 0 | +0 | 0 / +0 | 7 / +3 | 0 / +0 (704) | 0.9482 | 722 |

## pcbench

| Candidate | Boards | Unrouted | Copper | Mask | Fully connected | Partial | Completed | Deadline | CPU s | Max RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| main | 740 | 4359 | 812 | 13941 | 595 | 145 | 712 | 28 | 33196.90 | 295.2 |
| rounded-copper | 740 | 4323 | 814 | 13942 | 594 | 146 | 716 | 24 | 31857.29 | 295.1 |

| Baseline → candidate | Subset | Boards | U improved / worse | ΔU | Copper gainers / Δ | Mask gainers / Δ | Uncapped mask gainers / Δ (boards) | CPU ratio | Identical SES |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| main → rounded-copper | all | 740 | 7 / 5 | -36 | 2 / +2 | 12 / +1 | 1 / -1 (713) | 0.9596 | 720 |
| main → rounded-copper | both completed | 712 | 0 / 0 | +0 | 0 / +0 | 7 / +3 | 0 / +0 (694) | 0.9497 | 712 |

## local-kicad

| Candidate | Boards | Unrouted | Copper | Mask | Fully connected | Partial | Completed | Deadline | CPU s | Max RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| main | 11 | 230 | 58 | 0 | 7 | 4 | 10 | 1 | 773.68 | 134.3 |
| rounded-copper | 11 | 230 | 58 | 0 | 7 | 4 | 10 | 1 | 710.69 | 132.5 |

| Baseline → candidate | Subset | Boards | U improved / worse | ΔU | Copper gainers / Δ | Mask gainers / Δ | Uncapped mask gainers / Δ (boards) | CPU ratio | Identical SES |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| main → rounded-copper | all | 11 | 0 / 0 | +0 | 0 / +0 | 0 / +0 | 0 / +0 (11) | 0.9186 | 11 |
| main → rounded-copper | both completed | 10 | 0 / 0 | +0 | 0 / +0 | 0 / +0 | 0 / +0 (10) | 0.8670 | 10 |
