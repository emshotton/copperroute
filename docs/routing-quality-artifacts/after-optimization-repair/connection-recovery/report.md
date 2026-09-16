# Connection recovery subset experiment — 2026-09-13

Base: `66612acf`. 32 purposively selected boards (28 PCBench, four local fixtures), all scored by KiCad 10.0.4. Ten passes, 300-second deadline, one router thread, twelve jobs on workbench. This is an exploratory screen, not a full-corpus validation.

Positive deltas mean worse. Copper/routing and solder-mask findings are shown separately; both matter. CPU ratios include different routing work and deadline effects. Some boards reach the mask report cap; uncapped pairs are reported separately.

## Interpretation and activation

The trigger change works: individual recovery ran on 26 boards (102 trials, 17 retained), versus 11 boards (20 trials, two retained) for the whole-board control. Diagnostics recorded 1,043 guided connection attempts. Chess, Karabas revC, and Kinetoscope all activated individual recovery on pass 3; the whole-board control never activated on those three. The candidate uses fixed connected-component anchors (including pads, planes and user-fixed copper), not a permanent pair of pins: a merger/split resets that component's history.

Overall the candidate removes 22 external unrouted connections, but 29 non-timeout pairs worsen by three. Of the ten boards with different completeness, five improve and five regress. The three deadline-affected pairs contribute -25 connections (Karabas -23, Kinetoscope -7, chess +5). These boards now really exercise the algorithm, unlike earlier experiments, but one deadline-limited run cannot separate all timing effects from algorithm quality. One previously fully connected board (freeDSP BALANCED) finishes with two missing connections. This is a useful experimental mechanism, not yet a validated default or a full-corpus result. No commit or PR is being proposed from this screen alone.

The sole recorded mask increase is TinyTracker 199→200 at the report cap. Its final SES is byte-identical to baseline and it retained no recovery trials, so this is not evidence that the router introduced another mask defect. All 30 uncapped mask pairs are unchanged. Raw counts remain in the table for transparency.

All 96 cells were successfully scored by KiCad. Baseline unrouted/copper totals match the previous baseline on all 29 non-timeout shared boards. Full workspace tests passed before the release build. Large outputs remain on workbench; small reports and source fingerprints are copied locally.

## Investigated regressions

Original ground-truth reports for all affected boards show complete routing and zero routing DRC violations. No evidence justifies treating these regressions as designer outliers.

- Feather: the retained pass-5 trial improves internal incompletes 14→12, but the final internal count worsens 5→7 and KiCad worsens 26→28. GND remains 21 missing connections; the signal set changes. The candidate closes M0TX/USB_D+/D5 but opens M0RX/configSS/iceTX/D12. Original routing is complete (1,026.3151 mm, 57 vias); baseline is 869.2396 mm/37 vias and candidate 845.1513 mm/41 vias. Shorter incomplete routing is not an efficiency gain.
- HBR digital: both final internal counts are zero; KiCad reports GND 14→15. The original has zero DRC of any type, 2,115.3198 mm and 98 vias. Baseline is 2,044.0577 mm/23 vias, candidate 2,123.8362 mm/24 vias. The internal connectivity acceptance rule cannot see the extra external ground disconnection.
- Meshtastic: the candidate closes ESP32_RX, but KiCad GND disconnections increase 14→16, giving net +1. Original reference is fully connected. The connection trade is real, not an identified outlier exception.
- freeDSP BALANCED: the retained pass-7/8 trials improve internal counts locally, but final routing changes from complete to two open signals (`Net-(IC2-Pad17)` and `Net-(C62-Pad1)`). Baseline is 4,131.287 mm/68 vias versus candidate 4,006.0105 mm/70 vias; the complete original is 3,381.3136 mm/57 vias. Again the shorter candidate is incomplete.

The failures expose two distinct limitations: greedy pass-local improvements can lead to a poorer final topology, and internal connectivity does not see all post-import/zone-refill disconnections. The tracker resolves missed activation, but does not solve either acceptance problem.

| Variant / subset | Paired | Unrouted baseline → candidate (Δ) | U improved / regressed boards | Copper Δ / gaining boards | Mask Δ / gaining boards | CPU ratio | Fully connected baseline → candidate | Deadline baseline → candidate | Max RSS baseline → candidate (MiB) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| alternatives / all | 32 | 475 → 478 (+3) | 0 / 2 | -2 / 0 | +0 / 0 | 1.023× | 10 → 10 | 3 → 3 | 180.6 → 179.5 |
| alternatives / pcbench | 28 | 474 → 477 (+3) | 0 / 2 | -2 / 0 | +0 / 0 | 1.026× | 7 → 7 | 3 → 3 | 180.6 → 179.5 |
| alternatives / local | 4 | 1 → 1 (+0) | 0 / 0 | +0 / 0 | +0 / 0 | 0.991× | 3 → 3 | 0 → 0 | 49.8 → 56.6 |
| connections / all | 32 | 475 → 453 (-22) | 5 / 5 | -1 / 0 | +1 / 1 | 1.045× | 10 → 9 | 3 → 3 | 180.6 → 180.7 |
| connections / pcbench | 28 | 474 → 452 (-22) | 5 / 5 | -1 / 0 | +1 / 1 | 1.057× | 7 → 6 | 3 → 3 | 180.6 → 180.7 |
| connections / local | 4 | 1 → 1 (+0) | 0 / 0 | +0 / 0 | +0 / 0 | 0.928× | 3 → 3 | 0 → 0 | 49.8 → 57.8 |

## Deadline and report-cap controls

- alternatives: both sides completed normally on 29 boards: unrouted Δ +0, copper Δ +0, mask Δ +0. Mask counts below the cap on both sides: 30 boards, Δ +0, 0 gaining findings.
- connections: both sides completed normally on 29 boards: unrouted Δ +3, copper Δ +0, mask Δ +1. Mask counts below the cap on both sides: 30 boards, Δ +0, 0 gaining findings.

## Per-board changes

| Variant | Board | Unrouted | Copper | Mask | Either side reached deadline? |
|---|---|---:|---:|---:|---|
| alternatives | pcbench-real-time-chess_kfchess | 76 → 78 | 72 → 70 | 0 → 0 | True |
| alternatives | pcbench-kinetoscope_sram-bank | 111 → 112 | 0 → 0 | 0 → 0 | True |
| connections | pcbench-Feather-ICE40-PCB_feather_ice40 | 26 → 28 | 0 → 0 | 0 → 0 | False |
| connections | pcbench-Mouse_Mouse | 4 → 3 | 0 → 0 | 44 → 44 | False |
| connections | pcbench-hbr-mk2_hbr-mk2-digital | 14 → 15 | 0 → 0 | 0 → 0 | False |
| connections | pcbench-gb-hardware_GB-CARTPP-XC | 9 → 8 | 0 → 0 | 0 → 0 | False |
| connections | pcbench-project-hydra-meshtastic-pcb_meshtastic-diy | 24 → 25 | 5 → 5 | 2 → 2 | False |
| connections | pcbench-AIOsense_AIOsense | 18 → 17 | 0 → 0 | 0 → 0 | False |
| connections | pcbench-TinyTracker_ub-minimal | 3 → 3 | 0 → 0 | 199 → 200 | False |
| connections | pcbench-real-time-chess_kfchess | 76 → 81 | 72 → 71 | 0 → 0 | True |
| connections | pcbench-karabas-nano_karabas-nano-revC | 97 → 74 | 1 → 1 | 199 → 199 | True |
| connections | pcbench-kinetoscope_sram-bank | 111 → 104 | 0 → 0 | 0 → 0 | True |
| connections | pcbench-freeDSP-CLASSIC-SMD-BALANCED_FreeDSP_BAL | 0 → 2 | 0 → 0 | 0 → 0 | False |
