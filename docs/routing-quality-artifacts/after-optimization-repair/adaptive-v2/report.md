# Adaptive alternatives subset experiment — 2026-09-13

Base: `66612acf`. 32 purposively selected boards (28 PCBench, four local fixtures), all scored by KiCad 10.0.4. Ten passes, 300-second deadline, one router thread, twelve jobs on workbench. This is an exploratory screen, not a full-corpus validation.

Positive deltas mean worse. Copper/routing and solder-mask findings are shown separately; both matter. CPU ratios include different routing work and deadline effects. Some boards reach the mask report cap; uncapped pairs are reported separately.

No verified combined-routing quality gain was found. All 29 pairs that finished normally have byte-identical final SES output, including Feather and BMS. Eleven boards triggered 20 alternatives: 18 were rejected and two retained temporarily; even those two end at the baseline layout. The three deadline-affected boards account for all aggregate quality differences, and none activated combined recovery. Those differences must not be credited to this mechanism. Full workspace tests passed; this remains an unmerged experiment.

| Variant / subset | Paired | Unrouted baseline → candidate (Δ) | U improved / regressed boards | Copper Δ / gaining boards | Mask Δ / gaining boards | CPU ratio | Fully connected baseline → candidate | Deadline baseline → candidate | Max RSS baseline → candidate (MiB) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| alternatives / all | 32 | 477 → 428 (-49) | 2 / 1 | +3 / 1 | +0 / 0 | 0.994× | 10 → 10 | 3 → 2 | 180.1 → 180.1 |
| alternatives / pcbench | 28 | 476 → 427 (-49) | 2 / 1 | +3 / 1 | +0 / 0 | 1.020× | 7 → 7 | 3 → 2 | 180.1 → 180.1 |
| alternatives / local | 4 | 1 → 1 (+0) | 0 / 0 | +0 / 0 | +0 / 0 | 0.755× | 3 → 3 | 0 → 0 | 51.2 → 56.5 |

## Deadline and report-cap controls

- alternatives: both sides completed normally on 29 boards: unrouted Δ +0, copper Δ +0, mask Δ +0. Mask counts below the cap on both sides: 30 boards, Δ +0, 0 gaining findings.

## Per-board changes

| Variant | Board | Unrouted | Copper | Mask | Either side reached deadline? |
|---|---|---:|---:|---:|---|
| alternatives | pcbench-real-time-chess_kfchess | 78 → 71 | 70 → 73 | 0 → 0 | True |
| alternatives | pcbench-karabas-nano_karabas-nano-revC | 97 → 54 | 1 → 1 | 199 → 199 | True |
| alternatives | pcbench-kinetoscope_sram-bank | 111 → 112 | 0 → 0 | 0 → 0 | True |
