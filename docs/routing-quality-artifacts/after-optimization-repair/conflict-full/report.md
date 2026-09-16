# Full conflict-based search experiment — 2026-09-14

Base: `66612acf`. 751 eligible boards (740 PCBench, eleven local fixtures), all scored by KiCad 10.0.4. Ten passes, 300-second deadline, one router thread, twelve jobs on workbench. This evaluates all eligible KiCad boards in the imported workbench corpus; Java-only fixtures are excluded.

Baseline reused from `target-search-full-01`; candidate run `conflict-search-full-02`. Baseline binary, manifest, all 751 DSN inputs and valid scores were verified. CPU measurements come from separate runs. Positive deltas mean worse. Copper/routing and solder-mask findings are shown separately; both matter. CPU ratios include different routing work and deadline effects. Some boards reach the mask report cap; uncapped pairs are reported separately.

| Variant / subset | Paired | Unrouted baseline → candidate (Δ) | U improved / regressed boards | Copper Δ / gaining boards | Mask Δ / gaining boards | CPU ratio | Fully connected baseline → candidate | Deadline baseline → candidate | Max RSS baseline → candidate (MiB) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| conflicts / all | 751 | 4584 → 4529 (-55) | 16 / 18 | +0 / 7 | +68 / 20 | 1.033× | 602 → 601 | 27 → 26 | 295.2 → 295.4 |
| conflicts / pcbench | 740 | 4354 → 4300 (-54) | 15 / 18 | +0 / 7 | +68 / 20 | 1.035× | 595 → 593 | 26 → 25 | 295.2 → 295.4 |
| conflicts / local | 11 | 230 → 229 (-1) | 1 / 0 | +0 / 0 | +0 / 0 | 0.940× | 7 → 8 | 1 → 1 | 135.2 → 134.4 |

## Deadline and report-cap controls

- conflicts: both sides completed normally on 723 boards: unrouted Δ +0, copper Δ -5, mask Δ +68. Mask counts below the cap on both sides: 722 boards, Δ +3, 6 gaining findings.

## Per-board changes

| Variant | Board | Unrouted | Copper | Mask | Either side reached deadline? |
|---|---|---:|---:|---:|---|
| conflicts | pcbench-kitspace_USB-C-Screen-Adapter | 3 → 4 | 2 → 2 | 83 → 83 | False |
| conflicts | pcbench-BLDC-controller_BLDC_controller | 11 → 11 | 9 → 9 | 215 → 210 | False |
| conflicts | pcbench-Blitz_.C68 | 499 → 409 | 0 → 0 | 0 → 0 | True |
| conflicts | pcbench-Blitz_copy | 483 → 499 | 0 → 0 | 0 → 0 | True |
| conflicts | pcbench-CAL430FR_CAL430F | 0 → 1 | 0 → 0 | 0 → 0 | False |
| conflicts | pcbench-CATs-Eurosynth_Abakus_Main | 15 → 16 | 0 → 0 | 0 → 0 | False |
| conflicts | pcbench-CATs-Eurosynth_CGS_Analog_Switch_Matrix_Main | 8 → 12 | 0 → 0 | 0 → 0 | False |
| conflicts | pcbench-CATs-Eurosynth_MFOS_Eight_Stage_Phase_Shifter_-_Main | 20 → 19 | 0 → 0 | 0 → 0 | False |
| conflicts | pcbench-DC25_DC25 | 0 → 0 | 11 → 14 | 29 → 32 | False |
| conflicts | pcbench-Electronics-MainBoard_MainBoard | 0 → 0 | 0 → 0 | 201 → 206 | False |
| conflicts | pcbench-EncoderBoard_Enc_Pan_Led | 0 → 1 | 0 → 0 | 39 → 39 | False |
| conflicts | pcbench-FRM16_Relay_Module_I2C_Controller_relay_controller | 0 → 0 | 0 → 0 | 0 → 25 | False |
| conflicts | pcbench-Hardware_Playground_rpi_zero | 0 → 0 | 31 → 22 | 0 → 0 | False |
| conflicts | pcbench-LPC2148_Stick_LPC2148_stick | 3 → 0 | 3 → 2 | 182 → 199 | False |
| conflicts | pcbench-LPC2148_Stick__autosave-LPC2148_stick | 3 → 0 | 3 → 2 | 182 → 199 | False |
| conflicts | pcbench-Mechaduino-DR_Mechaduino DR 1.01 | 0 → 0 | 0 → 0 | 203 → 202 | False |
| conflicts | pcbench-MixSID_mixsid | 0 → 0 | 0 → 0 | 202 → 203 | False |
| conflicts | pcbench-Mouse_Mouse | 4 → 3 | 0 → 0 | 44 → 44 | False |
| conflicts | pcbench-NRC2016_banked_ram | 0 → 0 | 0 → 0 | 207 → 203 | False |
| conflicts | pcbench-NRC2016_usb_sio | 0 → 0 | 0 → 0 | 166 → 156 | False |
| conflicts | pcbench-Own-Mailbox-Hardware_eth | 1 → 1 | 0 → 0 | 204 → 209 | False |
| conflicts | pcbench-Own-Mailbox-Hardware_mailbox | 2 → 2 | 0 → 0 | 203 → 202 | False |
| conflicts | pcbench-Pi1541io_Pi1541io | 9 → 5 | 12 → 9 | 0 → 0 | False |
| conflicts | pcbench-RC6502-Apple-1-Replica_RC6502_Terminal | 8 → 10 | 0 → 0 | 0 → 0 | False |
| conflicts | pcbench-ReSDMAC_ReSDMAC | 121 → 111 | 0 → 0 | 0 → 0 | True |
| conflicts | pcbench-RoBoC_RoboticsMKII | 0 → 0 | 3 → 5 | 0 → 0 | False |
| conflicts | pcbench-Solare-BQ24210_Solare-BQ24210 | 1 → 0 | 0 → 0 | 14 → 14 | False |
| conflicts | pcbench-TLPHnodeV2_TLPHnodeV2 | 0 → 0 | 0 → 0 | 120 → 122 | False |
| conflicts | pcbench-avr-fuser-32_adapter | 0 → 0 | 0 → 0 | 204 → 211 | False |
| conflicts | pcbench-badge2016_Badge_init | 0 → 0 | 0 → 0 | 239 → 253 | False |
| conflicts | pcbench-balena-rover-wide-hat_resin-rover | 0 → 0 | 4 → 4 | 200 → 202 | False |
| conflicts | pcbench-bms-8s50-ic_bms-8s50-ic | 54 → 53 | 0 → 0 | 88 → 88 | True |
| conflicts | pcbench-bobc_MS-F100 | 0 → 0 | 0 → 0 | 189 → 169 | False |
| conflicts | pcbench-decelerator4030_decelerator4030 | 474 → 489 | 10 → 11 | 0 → 0 | True |
| conflicts | pcbench-domotics_base-board-arranged | 0 → 0 | 3 → 6 | 0 → 0 | False |
| conflicts | pcbench-dorkyboard_keyboard | 3 → 3 | 20 → 20 | 204 → 199 | False |
| conflicts | pcbench-epapercard_epapercard | 0 → 0 | 0 → 0 | 137 → 111 | False |
| conflicts | pcbench-esper_EsperDNS | 9 → 8 | 0 → 0 | 0 → 0 | False |
| conflicts | pcbench-free-of-charge_BMS | 0 → 1 | 0 → 0 | 142 → 143 | False |
| conflicts | pcbench-gb-hardware_GB-CARTPP-XC | 9 → 8 | 0 → 0 | 0 → 0 | False |
| conflicts | pcbench-hbr-mk2_hbr-mk2-bpfs | 7 → 9 | 0 → 0 | 0 → 0 | False |
| conflicts | pcbench-headstage-adapter_headstage adapter | 0 → 0 | 0 → 0 | 204 → 217 | False |
| conflicts | pcbench-karabas-nano_karabas-nano-revA | 29 → 30 | 0 → 0 | 198 → 198 | True |
| conflicts | pcbench-karabas-nano_karabas-nano-revB | 64 → 80 | 0 → 0 | 199 → 199 | True |
| conflicts | pcbench-karabas-nano_karabas-nano-revC | 81 → 94 | 1 → 1 | 200 → 200 | True |
| conflicts | pcbench-karabas-nano_karabas-nano-revG | 85 → 73 | 0 → 0 | 199 → 199 | True |
| conflicts | pcbench-kinetoscope_sram-bank | 112 → 114 | 0 → 0 | 0 → 0 | True |
| conflicts | pcbench-kitspace_40-channel-hv-switching-board | 0 → 0 | 0 → 0 | 202 → 201 | True |
| conflicts | pcbench-kitspace_T32_ref | 0 → 0 | 0 → 0 | 181 → 178 | True |
| conflicts | pcbench-kitspace_dropbot-front-panel | 0 → 0 | 0 → 0 | 208 → 209 | True |
| conflicts | pcbench-kitspace_dropbot_control_board | 0 → 0 | 0 → 0 | 202 → 205 | False |
| conflicts | pcbench-memsarray_mems_array | 0 → 0 | 0 → 0 | 201 → 205 | False |
| conflicts | pcbench-nonSNES_SNSP-CPU-1CHIP | 159 → 162 | 1 → 2 | 28 → 30 | True |
| conflicts | pcbench-pico-pi-rel_pico-pi | 0 → 0 | 0 → 0 | 203 → 200 | False |
| conflicts | pcbench-real-time-chess_kfchess | 78 → 76 | 70 → 73 | 0 → 0 | True |
| conflicts | pcbench-rp2040-dmxsun_baseboard_2slots | 0 → 1 | 8 → 7 | 0 → 0 | False |
| conflicts | pcbench-rp2040-dmxsun_baseboard_4slots | 0 → 1 | 3 → 5 | 4 → 4 | False |
| conflicts | pcbench-srambo_1_srambo_1 | 1 → 0 | 0 → 0 | 136 → 165 | False |
| conflicts | pcbench-timecircuits-hardware_BTTF-TimeCircuits | 0 → 0 | 0 → 0 | 205 → 204 | False |
| conflicts | pcbench-wavegen_rev3 | 12 → 12 | 0 → 0 | 201 → 202 | True |
| conflicts | pcbench-wavegen_waveform-generator-rev1 | 1 → 1 | 0 → 0 | 209 → 204 | False |
| conflicts | pcbench-wavegen_wavegen | 1 → 1 | 0 → 0 | 202 → 203 | False |
| conflicts | pcbench-zx-sizif-128_sizif128 | 0 → 1 | 0 → 0 | 12 → 12 | False |
| conflicts | pcbench-zx-sizif-xxs_sizif-xxs | 27 → 22 | 0 → 0 | 165 → 165 | True |
| conflicts | kicad-issue269-nowiresonpowerlayers--proba | 1 → 0 | 0 → 0 | 0 → 0 | True |
