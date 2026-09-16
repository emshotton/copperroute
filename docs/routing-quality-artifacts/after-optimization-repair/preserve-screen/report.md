# Preserved-neighbor conflict-search screen — 2026-09-14

Base: `66612acf`. 42 selected diagnostic boards, all scored by KiCad 10.0.4. Ten passes, 300-second deadline, one router thread, twelve jobs on workbench. Purposive subset; these totals are not a corpus-wide estimate.

Baseline reused from `target-search-full-01`; candidate run `conflict-preserve-subset-01`. Baseline binary, manifest, all 751 DSN inputs and valid scores were verified. CPU measurements come from separate runs. Positive deltas mean worse. Copper/routing and solder-mask findings are shown separately; both matter. CPU ratios include different routing work and deadline effects. Some boards reach the mask report cap; uncapped pairs are reported separately.

| Variant / subset | Paired | Unrouted baseline → candidate (Δ) | U improved / regressed boards | Copper Δ / gaining boards | Mask Δ / gaining boards | CPU ratio | Fully connected baseline → candidate | Deadline baseline → candidate | Max RSS baseline → candidate (MiB) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| preserve / all | 42 | 469 → 480 (+11) | 8 / 9 | +4 / 4 | -2 / 6 | 0.982× | 19 → 18 | 3 → 3 | 180.2 → 177.5 |
| preserve / pcbench | 38 | 468 → 479 (+11) | 8 / 9 | +4 / 4 | -2 / 6 | 0.991× | 16 → 15 | 3 → 3 | 180.2 → 177.5 |
| preserve / local | 4 | 1 → 1 (+0) | 0 / 0 | +0 / 0 | +0 / 0 | 0.861× | 3 → 3 | 0 → 0 | 51.7 → 48.0 |
| preservelate / all | 42 | 469 → 438 (-31) | 4 / 1 | +1 / 1 | -56 / 1 | 0.956× | 19 → 19 | 3 → 3 | 180.2 → 179.4 |
| preservelate / pcbench | 38 | 468 → 437 (-31) | 4 / 1 | +1 / 1 | -56 / 1 | 0.978× | 16 → 16 | 3 → 3 | 180.2 → 179.4 |
| preservelate / local | 4 | 1 → 1 (+0) | 0 / 0 | +0 / 0 | +0 / 0 | 0.639× | 3 → 3 | 0 → 0 | 51.7 → 51.7 |

## Deadline and report-cap controls

- preserve: both sides completed normally on 39 boards: unrouted Δ +7, copper Δ +0, mask Δ -1. Mask counts below the cap on both sides: 41 boards, Δ -1, 6 gaining findings.
- preservelate: both sides completed normally on 39 boards: unrouted Δ +0, copper Δ -2, mask Δ -55. Mask counts below the cap on both sides: 41 boards, Δ -55, 1 gaining findings.

## Per-board changes

| Variant | Board | Unrouted | Copper | Mask | Either side reached deadline? |
|---|---|---:|---:|---:|---|
| preserve | pcbench-CAL430FR_CAL430F | 0 → 1 | 0 → 0 | 0 → 0 | False |
| preserve | pcbench-CATs-Eurosynth_Abakus_Main | 15 → 20 | 0 → 0 | 0 → 0 | False |
| preserve | pcbench-CATs-Eurosynth_CGS_Analog_Switch_Matrix_Main | 8 → 11 | 0 → 0 | 0 → 0 | False |
| preserve | pcbench-Hardware_Playground_rpi_zero | 0 → 0 | 31 → 22 | 0 → 0 | False |
| preserve | pcbench-LPC2148_Stick_LPC2148_stick | 3 → 0 | 3 → 2 | 182 → 197 | False |
| preserve | pcbench-LPC2148_Stick__autosave-LPC2148_stick | 3 → 0 | 3 → 2 | 182 → 197 | False |
| preserve | pcbench-Mouse_Mouse | 4 → 1 | 0 → 0 | 44 → 44 | False |
| preserve | pcbench-NRC2016_usb_sio | 0 → 0 | 0 → 0 | 166 → 154 | False |
| preserve | pcbench-Pi1541io_Pi1541io | 9 → 9 | 12 → 17 | 0 → 0 | False |
| preserve | pcbench-RoBoC_RoboticsMKII | 0 → 0 | 3 → 7 | 0 → 0 | False |
| preserve | pcbench-Solare-BQ24210_Solare-BQ24210 | 1 → 0 | 0 → 0 | 14 → 14 | False |
| preserve | pcbench-TLPHnodeV2_TLPHnodeV2 | 0 → 0 | 0 → 0 | 120 → 122 | False |
| preserve | pcbench-bobc_MS-F100 | 0 → 0 | 0 → 0 | 189 → 163 | False |
| preserve | pcbench-epapercard_epapercard | 0 → 0 | 0 → 0 | 137 → 138 | False |
| preserve | pcbench-esper_EsperDNS | 9 → 7 | 0 → 0 | 0 → 0 | False |
| preserve | pcbench-free-of-charge_BMS | 0 → 1 | 0 → 0 | 142 → 143 | False |
| preserve | pcbench-freeDSP-CLASSIC-SMD-BALANCED_FreeDSP_BAL | 0 → 3 | 0 → 0 | 0 → 0 | False |
| preserve | pcbench-gb-hardware_GB-CARTPP-XC | 9 → 7 | 0 → 0 | 0 → 0 | False |
| preserve | pcbench-hbr-mk2_hbr-mk2-bpfs | 7 → 10 | 0 → 0 | 0 → 0 | False |
| preserve | pcbench-hbr-mk2_hbr-mk2-digital | 14 → 18 | 0 → 0 | 0 → 0 | False |
| preserve | pcbench-karabas-nano_karabas-nano-revC | 81 → 94 | 1 → 1 | 200 → 199 | True |
| preserve | pcbench-kinetoscope_sram-bank | 112 → 108 | 0 → 0 | 0 → 0 | True |
| preserve | pcbench-real-time-chess_kfchess | 78 → 73 | 70 → 74 | 0 → 0 | True |
| preserve | pcbench-rp2040-dmxsun_baseboard_2slots | 0 → 1 | 8 → 7 | 0 → 0 | False |
| preserve | pcbench-rp2040-dmxsun_baseboard_4slots | 0 → 0 | 3 → 6 | 4 → 4 | False |
| preserve | pcbench-srambo_1_srambo_1 | 1 → 1 | 0 → 0 | 136 → 139 | False |
| preservelate | pcbench-NRC2016_usb_sio | 0 → 0 | 0 → 0 | 166 → 110 | False |
| preservelate | pcbench-Pi1541io_Pi1541io | 9 → 9 | 12 → 10 | 0 → 0 | False |
| preservelate | pcbench-Solare-BQ24210_Solare-BQ24210 | 1 → 0 | 0 → 0 | 14 → 14 | False |
| preservelate | pcbench-free-of-charge_BMS | 0 → 2 | 0 → 0 | 142 → 143 | False |
| preservelate | pcbench-gb-hardware_GB-CARTPP-XC | 9 → 8 | 0 → 0 | 0 → 0 | False |
| preservelate | pcbench-karabas-nano_karabas-nano-revC | 81 → 57 | 1 → 1 | 200 → 199 | True |
| preservelate | pcbench-real-time-chess_kfchess | 78 → 71 | 70 → 73 | 0 → 0 | True |
