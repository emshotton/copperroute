# Conflict-search follow-up screen — 2026-09-14

Base: `66612acf`. 42 selected diagnostic boards, all scored by KiCad 10.0.4. Ten passes, 300-second deadline, one router thread, twelve jobs on workbench. Purposive subset; these totals are not a corpus-wide estimate.

Baseline reused from `target-search-full-01`; original conflict run `conflict-search-full-02`, new variants `conflict-followup-subset-01`. Baseline binary, manifest, all 751 DSN inputs and valid scores were verified. CPU measurements come from separate runs. Positive deltas mean worse. Copper/routing and solder-mask findings are shown separately; both matter. CPU ratios include different routing work and deadline effects. Some boards reach the mask report cap; uncapped pairs are reported separately.

| Variant / subset | Paired | Unrouted baseline → candidate (Δ) | U improved / regressed boards | Copper Δ / gaining boards | Mask Δ / gaining boards | CPU ratio | Fully connected baseline → candidate | Deadline baseline → candidate | Max RSS baseline → candidate (MiB) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| conflicts / all | 42 | 469 → 482 (+13) | 10 / 13 | -2 / 5 | +38 / 7 | 1.024× | 19 → 17 | 3 → 3 | 180.2 → 173.5 |
| conflicts / pcbench | 38 | 468 → 481 (+13) | 10 / 13 | -2 / 5 | +38 / 7 | 1.040× | 16 → 14 | 3 → 3 | 180.2 → 173.5 |
| conflicts / local | 4 | 1 → 1 (+0) | 0 / 0 | +0 / 0 | +0 / 0 | 0.788× | 3 → 3 | 0 → 0 | 51.7 → 51.8 |
| control / all | 42 | 469 → 454 (-15) | 2 / 0 | +2 / 1 | -1 / 0 | 0.987× | 19 → 19 | 3 → 3 | 180.2 → 179.9 |
| control / pcbench | 38 | 468 → 453 (-15) | 2 / 0 | +2 / 1 | -1 / 0 | 1.003× | 16 → 16 | 3 → 3 | 180.2 → 179.9 |
| control / local | 4 | 1 → 1 (+0) | 0 / 0 | +0 / 0 | +0 / 0 | 0.760× | 3 → 3 | 0 → 0 | 51.7 → 52.1 |
| reserved / all | 42 | 469 → 476 (+7) | 10 / 12 | +10 / 7 | +6 / 6 | 1.000× | 19 → 16 | 3 → 3 | 180.2 → 184.5 |
| reserved / pcbench | 38 | 468 → 475 (+7) | 10 / 12 | +10 / 7 | +6 / 6 | 1.001× | 16 → 13 | 3 → 3 | 180.2 → 184.5 |
| reserved / local | 4 | 1 → 1 (+0) | 0 / 0 | +0 / 0 | +0 / 0 | 0.990× | 3 → 3 | 0 → 0 | 51.7 → 47.3 |
| late / all | 42 | 469 → 482 (+13) | 2 / 3 | -2 / 1 | +1 / 1 | 0.997× | 19 → 19 | 3 → 3 | 180.2 → 180.2 |
| late / pcbench | 38 | 468 → 481 (+13) | 2 / 3 | -2 / 1 | +1 / 1 | 1.005× | 16 → 16 | 3 → 3 | 180.2 → 180.2 |
| late / local | 4 | 1 → 1 (+0) | 0 / 0 | +0 / 0 | +0 / 0 | 0.880× | 3 → 3 | 0 → 0 | 51.7 → 51.4 |
| reservedlate / all | 42 | 469 → 440 (-29) | 3 / 3 | -3 / 1 | -35 / 1 | 0.967× | 19 → 18 | 3 → 3 | 180.2 → 178.3 |
| reservedlate / pcbench | 38 | 468 → 439 (-29) | 3 / 3 | -3 / 1 | -35 / 1 | 0.987× | 16 → 15 | 3 → 3 | 180.2 → 178.3 |
| reservedlate / local | 4 | 1 → 1 (+0) | 0 / 0 | +0 / 0 | +0 / 0 | 0.695× | 3 → 3 | 0 → 0 | 51.7 → 51.5 |

## Deadline and report-cap controls

- conflicts: both sides completed normally on 39 boards: unrouted Δ +0, copper Δ -5, mask Δ +38. Mask counts below the cap on both sides: 39 boards, Δ +4, 5 gaining findings.
- control: both sides completed normally on 39 boards: unrouted Δ +0, copper Δ +0, mask Δ +0. Mask counts below the cap on both sides: 41 boards, Δ +0, 0 gaining findings.
- reserved: both sides completed normally on 39 boards: unrouted Δ +5, copper Δ -6, mask Δ +7. Mask counts below the cap on both sides: 39 boards, Δ -27, 4 gaining findings.
- late: both sides completed normally on 39 boards: unrouted Δ +4, copper Δ -6, mask Δ +1. Mask counts below the cap on both sides: 41 boards, Δ +1, 1 gaining findings.
- reservedlate: both sides completed normally on 39 boards: unrouted Δ +6, copper Δ -6, mask Δ -34. Mask counts below the cap on both sides: 41 boards, Δ -34, 1 gaining findings.

## Per-board changes

| Variant | Board | Unrouted | Copper | Mask | Either side reached deadline? |
|---|---|---:|---:|---:|---|
| conflicts | pcbench-kitspace_USB-C-Screen-Adapter | 3 → 4 | 2 → 2 | 83 → 83 | False |
| conflicts | pcbench-CAL430FR_CAL430F | 0 → 1 | 0 → 0 | 0 → 0 | False |
| conflicts | pcbench-CATs-Eurosynth_Abakus_Main | 15 → 16 | 0 → 0 | 0 → 0 | False |
| conflicts | pcbench-CATs-Eurosynth_CGS_Analog_Switch_Matrix_Main | 8 → 12 | 0 → 0 | 0 → 0 | False |
| conflicts | pcbench-CATs-Eurosynth_MFOS_Eight_Stage_Phase_Shifter_-_Main | 20 → 19 | 0 → 0 | 0 → 0 | False |
| conflicts | pcbench-DC25_DC25 | 0 → 0 | 11 → 14 | 29 → 32 | False |
| conflicts | pcbench-EncoderBoard_Enc_Pan_Led | 0 → 1 | 0 → 0 | 39 → 39 | False |
| conflicts | pcbench-FRM16_Relay_Module_I2C_Controller_relay_controller | 0 → 0 | 0 → 0 | 0 → 25 | False |
| conflicts | pcbench-Hardware_Playground_rpi_zero | 0 → 0 | 31 → 22 | 0 → 0 | False |
| conflicts | pcbench-LPC2148_Stick_LPC2148_stick | 3 → 0 | 3 → 2 | 182 → 199 | False |
| conflicts | pcbench-LPC2148_Stick__autosave-LPC2148_stick | 3 → 0 | 3 → 2 | 182 → 199 | False |
| conflicts | pcbench-Mouse_Mouse | 4 → 3 | 0 → 0 | 44 → 44 | False |
| conflicts | pcbench-NRC2016_usb_sio | 0 → 0 | 0 → 0 | 166 → 156 | False |
| conflicts | pcbench-Pi1541io_Pi1541io | 9 → 5 | 12 → 9 | 0 → 0 | False |
| conflicts | pcbench-RC6502-Apple-1-Replica_RC6502_Terminal | 8 → 10 | 0 → 0 | 0 → 0 | False |
| conflicts | pcbench-RoBoC_RoboticsMKII | 0 → 0 | 3 → 5 | 0 → 0 | False |
| conflicts | pcbench-Solare-BQ24210_Solare-BQ24210 | 1 → 0 | 0 → 0 | 14 → 14 | False |
| conflicts | pcbench-TLPHnodeV2_TLPHnodeV2 | 0 → 0 | 0 → 0 | 120 → 122 | False |
| conflicts | pcbench-bobc_MS-F100 | 0 → 0 | 0 → 0 | 189 → 169 | False |
| conflicts | pcbench-domotics_base-board-arranged | 0 → 0 | 3 → 6 | 0 → 0 | False |
| conflicts | pcbench-epapercard_epapercard | 0 → 0 | 0 → 0 | 137 → 111 | False |
| conflicts | pcbench-esper_EsperDNS | 9 → 8 | 0 → 0 | 0 → 0 | False |
| conflicts | pcbench-free-of-charge_BMS | 0 → 1 | 0 → 0 | 142 → 143 | False |
| conflicts | pcbench-gb-hardware_GB-CARTPP-XC | 9 → 8 | 0 → 0 | 0 → 0 | False |
| conflicts | pcbench-hbr-mk2_hbr-mk2-bpfs | 7 → 9 | 0 → 0 | 0 → 0 | False |
| conflicts | pcbench-karabas-nano_karabas-nano-revC | 81 → 94 | 1 → 1 | 200 → 200 | True |
| conflicts | pcbench-kinetoscope_sram-bank | 112 → 114 | 0 → 0 | 0 → 0 | True |
| conflicts | pcbench-real-time-chess_kfchess | 78 → 76 | 70 → 73 | 0 → 0 | True |
| conflicts | pcbench-rp2040-dmxsun_baseboard_2slots | 0 → 1 | 8 → 7 | 0 → 0 | False |
| conflicts | pcbench-rp2040-dmxsun_baseboard_4slots | 0 → 1 | 3 → 5 | 4 → 4 | False |
| conflicts | pcbench-srambo_1_srambo_1 | 1 → 0 | 0 → 0 | 136 → 165 | False |
| conflicts | pcbench-zx-sizif-128_sizif128 | 0 → 1 | 0 → 0 | 12 → 12 | False |
| control | pcbench-karabas-nano_karabas-nano-revC | 81 → 68 | 1 → 1 | 200 → 199 | True |
| control | pcbench-real-time-chess_kfchess | 78 → 76 | 70 → 72 | 0 → 0 | True |
| reserved | pcbench-CAL430FR_CAL430F | 0 → 1 | 0 → 0 | 0 → 0 | False |
| reserved | pcbench-CATs-Eurosynth_CGS_Analog_Switch_Matrix_Main | 8 → 12 | 0 → 0 | 0 → 0 | False |
| reserved | pcbench-CATs-Eurosynth_MFOS_Eight_Stage_Phase_Shifter_-_Main | 20 → 19 | 0 → 0 | 0 → 0 | False |
| reserved | pcbench-DC25_DC25 | 0 → 0 | 11 → 7 | 29 → 29 | False |
| reserved | pcbench-EncoderBoard_Enc_Pan_Led | 0 → 1 | 0 → 0 | 39 → 39 | False |
| reserved | pcbench-FRM16_Relay_Module_I2C_Controller_relay_controller | 0 → 0 | 0 → 0 | 0 → 35 | False |
| reserved | pcbench-Hardware_Playground_rpi_zero | 0 → 0 | 31 → 22 | 0 → 0 | False |
| reserved | pcbench-LPC2148_Stick_LPC2148_stick | 3 → 1 | 3 → 5 | 182 → 199 | False |
| reserved | pcbench-LPC2148_Stick__autosave-LPC2148_stick | 3 → 1 | 3 → 5 | 182 → 199 | False |
| reserved | pcbench-Mouse_Mouse | 4 → 1 | 0 → 0 | 44 → 44 | False |
| reserved | pcbench-NRC2016_usb_sio | 0 → 0 | 0 → 0 | 166 → 128 | False |
| reserved | pcbench-Pi1541io_Pi1541io | 9 → 10 | 12 → 9 | 0 → 0 | False |
| reserved | pcbench-RC6502-Apple-1-Replica_RC6502_Terminal | 8 → 9 | 0 → 0 | 0 → 0 | False |
| reserved | pcbench-RoBoC_RoboticsMKII | 0 → 0 | 3 → 4 | 0 → 0 | False |
| reserved | pcbench-Solare-BQ24210_Solare-BQ24210 | 1 → 0 | 0 → 0 | 14 → 14 | False |
| reserved | pcbench-TLPHnodeV2_TLPHnodeV2 | 0 → 0 | 0 → 0 | 120 → 122 | False |
| reserved | pcbench-bobc_MS-F100 | 0 → 0 | 0 → 0 | 189 → 169 | False |
| reserved | pcbench-domotics_base-board-arranged | 0 → 0 | 3 → 6 | 0 → 0 | False |
| reserved | pcbench-epapercard_epapercard | 0 → 0 | 0 → 0 | 137 → 114 | False |
| reserved | pcbench-esper_EsperDNS | 9 → 7 | 0 → 0 | 0 → 0 | False |
| reserved | pcbench-free-of-charge_BMS | 0 → 1 | 0 → 0 | 142 → 143 | False |
| reserved | pcbench-freeDSP-CLASSIC-SMD-BALANCED_FreeDSP_BAL | 0 → 2 | 0 → 0 | 0 → 0 | False |
| reserved | pcbench-gb-hardware_GB-CARTPP-XC | 9 → 7 | 0 → 0 | 0 → 0 | False |
| reserved | pcbench-hbr-mk2_hbr-mk2-bpfs | 7 → 10 | 0 → 0 | 0 → 0 | False |
| reserved | pcbench-hbr-mk2_hbr-mk2-digital | 14 → 17 | 0 → 0 | 0 → 0 | False |
| reserved | pcbench-karabas-nano_karabas-nano-revC | 81 → 98 | 1 → 1 | 200 → 199 | True |
| reserved | pcbench-kinetoscope_sram-bank | 112 → 104 | 0 → 1 | 0 → 0 | True |
| reserved | pcbench-project-hydra-meshtastic-pcb_meshtastic-diy | 24 → 25 | 5 → 5 | 2 → 1 | False |
| reserved | pcbench-real-time-chess_kfchess | 78 → 71 | 70 → 85 | 0 → 0 | True |
| reserved | pcbench-rp2040-dmxsun_baseboard_2slots | 0 → 1 | 8 → 7 | 0 → 0 | False |
| reserved | pcbench-rp2040-dmxsun_baseboard_4slots | 0 → 0 | 3 → 6 | 4 → 4 | False |
| reserved | pcbench-srambo_1_srambo_1 | 1 → 0 | 0 → 0 | 136 → 153 | False |
| late | pcbench-Pi1541io_Pi1541io | 9 → 10 | 12 → 6 | 0 → 0 | False |
| late | pcbench-Solare-BQ24210_Solare-BQ24210 | 1 → 0 | 0 → 0 | 14 → 14 | False |
| late | pcbench-free-of-charge_BMS | 0 → 4 | 0 → 0 | 142 → 143 | False |
| late | pcbench-karabas-nano_karabas-nano-revC | 81 → 80 | 1 → 1 | 200 → 200 | True |
| late | pcbench-real-time-chess_kfchess | 78 → 88 | 70 → 74 | 0 → 0 | True |
| reservedlate | pcbench-NRC2016_usb_sio | 0 → 0 | 0 → 0 | 166 → 131 | False |
| reservedlate | pcbench-Pi1541io_Pi1541io | 9 → 10 | 12 → 6 | 0 → 0 | False |
| reservedlate | pcbench-Solare-BQ24210_Solare-BQ24210 | 1 → 0 | 0 → 0 | 14 → 14 | False |
| reservedlate | pcbench-free-of-charge_BMS | 0 → 3 | 0 → 0 | 142 → 143 | False |
| reservedlate | pcbench-karabas-nano_karabas-nano-revC | 81 → 53 | 1 → 1 | 200 → 199 | True |
| reservedlate | pcbench-real-time-chess_kfchess | 78 → 71 | 70 → 73 | 0 → 0 | True |
| reservedlate | pcbench-zx-sizif-128_sizif128 | 0 → 3 | 0 → 0 | 12 → 12 | False |
