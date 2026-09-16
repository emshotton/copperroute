# Adaptive recovery subset experiment — 2026-09-13

Base: `66612acf`. 32 purposively selected boards (28 PCBench, four local fixtures), all scored by KiCad 10.0.4. Ten passes, 300-second deadline, one router thread, twelve jobs on workbench. This is an exploratory screen, not a full-corpus validation.

Positive deltas mean worse. Copper/routing and solder-mask findings are shown separately; both matter. CPU ratios include different routing work and deadline effects. Some boards reach the mask report cap; uncapped pairs are reported separately.

The first combined-recovery version is rejected. On the 29 normally finishing pairs it adds eight unrouted connections and five mask findings (Feather and BMS regress). Fourteen boards triggered 26 trials, with 23 retained locally and three rolled back. The three apparent improving boards are deadline-limited and never activated recovery, so the aggregate gain is not evidence for the algorithm. Full workspace tests passed; no production commit was made.

| Variant / subset | Paired | Unrouted baseline → candidate (Δ) | U improved / regressed boards | Copper Δ / gaining boards | Mask Δ / gaining boards | CPU ratio | Fully connected baseline → candidate | Deadline baseline → candidate | Max RSS baseline → candidate (MiB) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| adaptive / all | 32 | 494 → 457 (-37) | 3 / 2 | -3 / 0 | +5 / 1 | 0.973× | 10 → 9 | 3 → 3 | 178.5 → 179.3 |
| adaptive / pcbench | 28 | 493 → 456 (-37) | 3 / 2 | -3 / 0 | +5 / 1 | 0.986× | 7 → 6 | 3 → 3 | 178.5 → 179.3 |
| adaptive / local | 4 | 1 → 1 (+0) | 0 / 0 | +0 / 0 | +0 / 0 | 0.847× | 3 → 3 | 0 → 0 | 51.2 → 54.3 |

## Deadline and report-cap controls

- adaptive: both sides completed normally on 29 boards: unrouted Δ +8, copper Δ +0, mask Δ +5. Mask counts below the cap on both sides: 30 boards, Δ +5, 1 gaining findings.

## Per-board changes

| Variant | Board | Unrouted | Copper | Mask | Either side reached deadline? |
|---|---|---:|---:|---:|---|
| adaptive | pcbench-Feather-ICE40-PCB_feather_ice40 | 26 → 30 | 0 → 0 | 0 → 0 | False |
| adaptive | pcbench-real-time-chess_kfchess | 90 → 76 | 75 → 72 | 0 → 0 | True |
| adaptive | pcbench-karabas-nano_karabas-nano-revC | 96 → 70 | 1 → 1 | 199 → 199 | True |
| adaptive | pcbench-kinetoscope_sram-bank | 117 → 112 | 0 → 0 | 0 → 0 | True |
| adaptive | pcbench-free-of-charge_BMS | 0 → 4 | 0 → 0 | 142 → 147 | False |
