# Terminal conflict-search screen — 2026-09-14

Base: `66612acf`. 42 selected diagnostic boards, all scored by KiCad 10.0.4. Ten passes, 300-second deadline, one router thread, twelve jobs on workbench. Purposive subset; these totals are not a corpus-wide estimate.

Baseline reused from `target-search-full-01`; candidate run `conflict-terminal-subset-01`. Baseline binary, manifest, all 751 DSN inputs and valid scores were verified. CPU measurements come from separate runs. Positive deltas mean worse. Copper/routing and solder-mask findings are shown separately; both matter. CPU ratios include different routing work and deadline effects. Some boards reach the mask report cap; uncapped pairs are reported separately.

| Variant / subset | Paired | Unrouted baseline → candidate (Δ) | U improved / regressed boards | Copper Δ / gaining boards | Mask Δ / gaining boards | CPU ratio | Fully connected baseline → candidate | Deadline baseline → candidate | Max RSS baseline → candidate (MiB) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| control / all | 42 | 469 → 464 (-5) | 3 / 0 | +2 / 1 | -1 / 0 | 0.988× | 19 → 19 | 3 → 3 | 180.2 → 179.2 |
| control / pcbench | 38 | 468 → 463 (-5) | 3 / 0 | +2 / 1 | -1 / 0 | 1.002× | 16 → 16 | 3 → 3 | 180.2 → 179.2 |
| control / local | 4 | 1 → 1 (+0) | 0 / 0 | +0 / 0 | +0 / 0 | 0.782× | 3 → 3 | 0 → 0 | 51.7 → 51.4 |
| final / all | 42 | 469 → 475 (+6) | 2 / 1 | +4 / 1 | -1 / 0 | 0.992× | 19 → 20 | 3 → 3 | 180.2 → 182.1 |
| final / pcbench | 38 | 468 → 474 (+6) | 2 / 1 | +4 / 1 | -1 / 0 | 1.003× | 16 → 17 | 3 → 3 | 180.2 → 182.1 |
| final / local | 4 | 1 → 1 (+0) | 0 / 0 | +0 / 0 | +0 / 0 | 0.836× | 3 → 3 | 0 → 0 | 51.7 → 51.3 |
| reservedfinal / all | 42 | 469 → 435 (-34) | 3 / 0 | +3 / 1 | -1 / 0 | 0.975× | 19 → 20 | 3 → 3 | 180.2 → 179.6 |
| reservedfinal / pcbench | 38 | 468 → 434 (-34) | 3 / 0 | +3 / 1 | -1 / 0 | 0.997× | 16 → 17 | 3 → 3 | 180.2 → 179.6 |
| reservedfinal / local | 4 | 1 → 1 (+0) | 0 / 0 | +0 / 0 | +0 / 0 | 0.665× | 3 → 3 | 0 → 0 | 51.7 → 51.5 |

## Deadline and report-cap controls

- control: both sides completed normally on 39 boards: unrouted Δ +0, copper Δ +0, mask Δ +0. Mask counts below the cap on both sides: 41 boards, Δ +0, 0 gaining findings.
- final: both sides completed normally on 39 boards: unrouted Δ -1, copper Δ +0, mask Δ +0. Mask counts below the cap on both sides: 41 boards, Δ +0, 0 gaining findings.
- reservedfinal: both sides completed normally on 39 boards: unrouted Δ -1, copper Δ +0, mask Δ +0. Mask counts below the cap on both sides: 41 boards, Δ +0, 0 gaining findings.

## Per-board changes

| Variant | Board | Unrouted | Copper | Mask | Either side reached deadline? |
|---|---|---:|---:|---:|---|
| control | pcbench-karabas-nano_karabas-nano-revC | 81 → 80 | 1 → 1 | 200 → 199 | True |
| control | pcbench-kinetoscope_sram-bank | 112 → 110 | 0 → 0 | 0 → 0 | True |
| control | pcbench-real-time-chess_kfchess | 78 → 76 | 70 → 72 | 0 → 0 | True |
| final | pcbench-Solare-BQ24210_Solare-BQ24210 | 1 → 0 | 0 → 0 | 14 → 14 | False |
| final | pcbench-karabas-nano_karabas-nano-revC | 81 → 78 | 1 → 1 | 200 → 199 | True |
| final | pcbench-real-time-chess_kfchess | 78 → 88 | 70 → 74 | 0 → 0 | True |
| reservedfinal | pcbench-Solare-BQ24210_Solare-BQ24210 | 1 → 0 | 0 → 0 | 14 → 14 | False |
| reservedfinal | pcbench-karabas-nano_karabas-nano-revC | 81 → 55 | 1 → 1 | 200 → 199 | True |
| reservedfinal | pcbench-real-time-chess_kfchess | 78 → 71 | 70 → 73 | 0 → 0 | True |
