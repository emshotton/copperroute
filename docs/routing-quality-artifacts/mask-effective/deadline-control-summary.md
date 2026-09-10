One pass; optimizer disabled; 1200-second cap. All99 cells completed. Not a replacement for the10-pass evaluation.

# Routing validation

KiCad only. Copper and mask findings are reported separately. A completed run may still have unrouted connections. Both-completed comparisons exclude external timeouts and require internal COMPLETED state on both sides. RSS is each board's process peak, not aggregate host memory. Mask reports at 199 findings may be capped. Single runs do not establish a speed improvement.

## all

| Candidate | Boards | Unrouted | Copper | Mask | Fully connected | Partial | Completed | Deadline | CPU s | Max RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| main | 33 | 3774 | 195 | 3196 | 0 | 33 | 33 | 0 | 4176.49 | 108.2 |
| mask-layers | 33 | 3774 | 195 | 3213 | 0 | 33 | 33 | 0 | 4230.21 | 105.8 |
| mask-effective | 33 | 3774 | 195 | 3193 | 0 | 33 | 33 | 0 | 3729.66 | 107.6 |

| Baseline → candidate | Subset | Boards | U improved / worse | ΔU | Copper gainers / Δ | Mask gainers / Δ | Uncapped mask gainers / Δ (boards) | CPU ratio | Identical SES |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| main → mask-layers | both completed | 33 | 0 / 0 | +0 | 0 / +0 | 4 / +17 | 0 / +0 (23) | 1.0129 | 33 |
| main → mask-effective | both completed | 33 | 0 / 0 | +0 | 0 / +0 | 3 / -3 | 0 / +0 (23) | 0.8930 | 33 |

## pcbench

| Candidate | Boards | Unrouted | Copper | Mask | Fully connected | Partial | Completed | Deadline | CPU s | Max RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| main | 31 | 3614 | 143 | 3196 | 0 | 31 | 31 | 0 | 4058.08 | 108.2 |
| mask-layers | 31 | 3614 | 143 | 3213 | 0 | 31 | 31 | 0 | 4121.20 | 105.8 |
| mask-effective | 31 | 3614 | 143 | 3193 | 0 | 31 | 31 | 0 | 3602.38 | 107.6 |

| Baseline → candidate | Subset | Boards | U improved / worse | ΔU | Copper gainers / Δ | Mask gainers / Δ | Uncapped mask gainers / Δ (boards) | CPU ratio | Identical SES |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| main → mask-layers | both completed | 31 | 0 / 0 | +0 | 0 / +0 | 4 / +17 | 0 / +0 (21) | 1.0156 | 31 |
| main → mask-effective | both completed | 31 | 0 / 0 | +0 | 0 / +0 | 3 / -3 | 0 / +0 (21) | 0.8877 | 31 |

## local-kicad

| Candidate | Boards | Unrouted | Copper | Mask | Fully connected | Partial | Completed | Deadline | CPU s | Max RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| main | 2 | 160 | 52 | 0 | 0 | 2 | 2 | 0 | 118.41 | 55.0 |
| mask-layers | 2 | 160 | 52 | 0 | 0 | 2 | 2 | 0 | 109.01 | 55.0 |
| mask-effective | 2 | 160 | 52 | 0 | 0 | 2 | 2 | 0 | 127.28 | 54.9 |

| Baseline → candidate | Subset | Boards | U improved / worse | ΔU | Copper gainers / Δ | Mask gainers / Δ | Uncapped mask gainers / Δ (boards) | CPU ratio | Identical SES |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| main → mask-layers | both completed | 2 | 0 / 0 | +0 | 0 / +0 | 0 / +0 | 0 / +0 (2) | 0.9206 | 2 |
| main → mask-effective | both completed | 2 | 0 / 0 | +0 | 0 / +0 | 0 / +0 | 0 / +0 (2) | 1.0749 | 2 |
