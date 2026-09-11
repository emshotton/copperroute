# Routing validation

KiCad only. Copper and mask findings are reported separately. A completed run may still have unrouted connections. Both-completed comparisons exclude external timeouts and require internal COMPLETED state on both sides. RSS is each board's process peak, not aggregate host memory. Mask reports at 199 findings may be capped. Single runs do not establish a speed improvement.

## all

| Candidate | Boards | Unrouted | Copper | Mask | Fully connected | Partial | Completed | Deadline | CPU s | Max RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| main | 13 | 2225 | 82 | 1460 | 0 | 13 | 13 | 0 | 2157.82 | 101.6 |
| rounded-copper | 13 | 2225 | 82 | 1459 | 0 | 13 | 13 | 0 | 1875.01 | 102.4 |

| Baseline → candidate | Subset | Boards | U improved / worse | ΔU | Copper gainers / Δ | Mask gainers / Δ | Uncapped mask gainers / Δ (boards) | CPU ratio | Identical SES |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| main → rounded-copper | all | 13 | 0 / 0 | +0 | 0 / +0 | 0 / -1 | 0 / +0 (9) | 0.8689 | 13 |
| main → rounded-copper | both completed | 13 | 0 / 0 | +0 | 0 / +0 | 0 / -1 | 0 / +0 (9) | 0.8689 | 13 |

## pcbench

| Candidate | Boards | Unrouted | Copper | Mask | Fully connected | Partial | Completed | Deadline | CPU s | Max RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| main | 13 | 2225 | 82 | 1460 | 0 | 13 | 13 | 0 | 2157.82 | 101.6 |
| rounded-copper | 13 | 2225 | 82 | 1459 | 0 | 13 | 13 | 0 | 1875.01 | 102.4 |

| Baseline → candidate | Subset | Boards | U improved / worse | ΔU | Copper gainers / Δ | Mask gainers / Δ | Uncapped mask gainers / Δ (boards) | CPU ratio | Identical SES |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| main → rounded-copper | all | 13 | 0 / 0 | +0 | 0 / +0 | 0 / -1 | 0 / +0 (9) | 0.8689 | 13 |
| main → rounded-copper | both completed | 13 | 0 / 0 | +0 | 0 / +0 | 0 / -1 | 0 / +0 (9) | 0.8689 | 13 |

## local-kicad

| Candidate | Boards | Unrouted | Copper | Mask | Fully connected | Partial | Completed | Deadline | CPU s | Max RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| main | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0.00 | 0.0 |
| rounded-copper | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0.00 | 0.0 |

| Baseline → candidate | Subset | Boards | U improved / worse | ΔU | Copper gainers / Δ | Mask gainers / Δ | Uncapped mask gainers / Δ (boards) | CPU ratio | Identical SES |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| main → rounded-copper | all | 0 | 0 / 0 | +0 | 0 / +0 | 0 / +0 | 0 / +0 (0) | n/a | 0 |
| main → rounded-copper | both completed | 0 | 0 / 0 | +0 | 0 / +0 | 0 / +0 | 0 / +0 (0) | n/a | 0 |
