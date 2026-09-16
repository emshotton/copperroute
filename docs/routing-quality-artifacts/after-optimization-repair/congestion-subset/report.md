# Congestion subset experiment — 2026-09-13

Base: `66612acf`. 32 purposively selected boards (28 PCBench, four local fixtures), all scored by KiCad 10.0.4. Ten passes, 300-second deadline, one router thread, four jobs on workbench. This is an exploratory screen, not a full-corpus validation.

Positive deltas mean worse. Copper/routing and solder-mask findings are shown separately; both matter. CPU ratios include different routing work and deadline effects. Two baseline boards have mask counts at the report cap.

| Variant / subset | Paired | Unrouted baseline → candidate (Δ) | U improved / regressed boards | Copper Δ / gaining boards | Mask Δ / gaining boards | CPU ratio | Fully connected baseline → candidate | Deadline baseline → candidate | Max RSS baseline → candidate (MiB) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| demand / all | 32 | 482 → 468 (-14) | 8 / 12 | +8 / 4 | +23 / 2 | 0.980× | 10 → 10 | 3 → 3 | 178.2 → 144.9 |
| demand / pcbench | 28 | 481 → 466 (-15) | 8 / 11 | +8 / 4 | +23 / 2 | 1.001× | 7 → 8 | 3 → 3 | 178.2 → 144.9 |
| demand / local | 4 | 1 → 2 (+1) | 0 / 1 | +0 / 0 | +0 / 0 | 0.795× | 3 → 2 | 0 → 0 | 51.5 → 46.1 |
| targets / all | 32 | 482 → 432 (-50) | 5 / 6 | +6 / 2 | +69 / 3 | 0.795× | 10 → 10 | 3 → 2 | 178.2 → 117.7 |
| targets / pcbench | 28 | 481 → 431 (-50) | 5 / 6 | +6 / 2 | +69 / 3 | 0.827× | 7 → 7 | 3 → 2 | 178.2 → 117.7 |
| targets / local | 4 | 1 → 1 (+0) | 0 / 0 | +0 / 0 | +0 / 0 | 0.508× | 3 → 3 | 0 → 0 | 51.5 → 45.0 |

## Deadline and report-cap controls

- demand: both sides completed normally on 29 boards: unrouted Δ +18, copper Δ +7, mask Δ +23. Mask counts below the cap on both sides: 30 boards, Δ +25, 2 gaining findings.
- targets: both sides completed normally on 29 boards: unrouted Δ +6, copper Δ +2, mask Δ +69. Mask counts below the cap on both sides: 30 boards, Δ +70, 3 gaining findings.

## Per-board changes

| Variant | Board | Unrouted | Copper | Mask | Either side reached deadline? |
|---|---|---:|---:|---:|---|
| demand | pcbench-Feather-ICE40-PCB_feather_ice40 | 26 → 29 | 0 → 0 | 0 → 0 | False |
| demand | pcbench-srambo_1_srambo_1 | 1 → 0 | 0 → 0 | 136 → 150 | False |
| demand | pcbench-kitspace_sympetrum-v2%20NFF1.1 | 13 → 13 | 0 → 0 | 171 → 185 | False |
| demand | pcbench-8bit-cpu_programming_interface | 5 → 4 | 0 → 0 | 1 → 0 | False |
| demand | pcbench-EnvOpenPico_EliteMicro2040 | 13 → 17 | 0 → 0 | 0 → 0 | False |
| demand | pcbench-Mouse_Mouse | 4 → 0 | 0 → 0 | 44 → 44 | False |
| demand | pcbench-RC6502-Apple-1-Replica_RC6502_Terminal | 8 → 9 | 0 → 0 | 0 → 0 | False |
| demand | pcbench-TK44_TK44 | 15 → 15 | 1 → 6 | 0 → 0 | False |
| demand | pcbench-gb-hardware_GB-CARTPP-XC | 9 → 7 | 0 → 0 | 0 → 0 | False |
| demand | pcbench-project-hydra-meshtastic-pcb_meshtastic-diy | 24 → 28 | 5 → 6 | 2 → 1 | False |
| demand | pcbench-AIOsense_AIOsense | 18 → 21 | 0 → 1 | 0 → 0 | False |
| demand | pcbench-antdroid-board_antdroid-board | 18 → 20 | 0 → 0 | 0 → 0 | False |
| demand | pcbench-ChirpHardware_chirp | 1 → 0 | 0 → 0 | 29 → 29 | False |
| demand | pcbench-Doorman_doorman_slot | 6 → 7 | 0 → 0 | 0 → 0 | False |
| demand | pcbench-TinyTracker_ub-minimal | 3 → 3 | 0 → 0 | 200 → 198 | False |
| demand | pcbench-esper_EsperDNS | 9 → 10 | 0 → 0 | 0 → 0 | False |
| demand | pcbench-kitspace_USB-C-Screen-Adapter | 3 → 4 | 2 → 2 | 83 → 83 | False |
| demand | pcbench-real-time-chess_kfchess | 84 → 67 | 73 → 73 | 0 → 0 | True |
| demand | pcbench-karabas-nano_karabas-nano-revC | 97 → 89 | 1 → 1 | 199 → 199 | True |
| demand | pcbench-kinetoscope_sram-bank | 110 → 103 | 0 → 1 | 0 → 0 | True |
| demand | pcbench-freeDSP-CLASSIC-SMD-BALANCED_FreeDSP_BAL | 0 → 5 | 0 → 0 | 0 → 0 | False |
| demand | pcbench-free-of-charge_BMS | 0 → 1 | 0 → 0 | 142 → 142 | False |
| demand | kicad-issue180-test--test | 0 → 1 | 0 → 0 | 0 → 0 | False |
| demand | pcbench-TLPHnodeV2_TLPHnodeV2 | 0 → 0 | 0 → 0 | 120 → 119 | False |
| targets | pcbench-Feather-ICE40-PCB_feather_ice40 | 26 → 27 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-srambo_1_srambo_1 | 1 → 0 | 0 → 0 | 136 → 177 | False |
| targets | pcbench-kitspace_sympetrum-v2%20NFF1.1 | 13 → 13 | 0 → 0 | 171 → 173 | False |
| targets | pcbench-8bit-cpu_programming_interface | 5 → 3 | 0 → 0 | 1 → 32 | False |
| targets | pcbench-project-hydra-meshtastic-pcb_meshtastic-diy | 24 → 28 | 5 → 7 | 2 → 2 | False |
| targets | pcbench-AIOsense_AIOsense | 18 → 19 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-TinyTracker_ub-minimal | 3 → 3 | 0 → 0 | 200 → 199 | False |
| targets | pcbench-esper_EsperDNS | 9 → 10 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-kitspace_USB-C-Screen-Adapter | 3 → 4 | 2 → 2 | 83 → 83 | False |
| targets | pcbench-real-time-chess_kfchess | 84 → 69 | 73 → 77 | 0 → 0 | True |
| targets | pcbench-karabas-nano_karabas-nano-revC | 97 → 59 | 1 → 1 | 199 → 199 | True |
| targets | pcbench-kinetoscope_sram-bank | 110 → 107 | 0 → 0 | 0 → 0 | True |
| targets | pcbench-freeDSP-CLASSIC-SMD-BALANCED_FreeDSP_BAL | 0 → 1 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-TLPHnodeV2_TLPHnodeV2 | 0 → 0 | 0 → 0 | 120 → 116 | False |
