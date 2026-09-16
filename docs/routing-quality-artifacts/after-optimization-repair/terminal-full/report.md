# Full terminal conflict-search evaluation — 2026-09-14

Base: `66612acf`. 751 eligible KiCad boards (740 PCBench, eleven local), all scored by KiCad 10.0.4. Ten passes, 300-second deadline, one router thread, twelve jobs on workbench. All eligible imported KiCad boards; no Java fixtures.

Baseline reused from `target-search-full-01`; candidate run `conflict-terminal-full-01`. Baseline binary, manifest, all 751 DSN inputs and valid scores were verified. CPU measurements come from separate runs. Positive deltas mean worse. Copper/routing and solder-mask findings are shown separately; both matter. CPU ratios include different routing work and deadline effects. Some boards reach the mask report cap; uncapped pairs are reported separately.

| Variant / subset | Paired | Unrouted baseline → candidate (Δ) | U improved / regressed boards | Copper Δ / gaining boards | Mask Δ / gaining boards | CPU ratio | Fully connected baseline → candidate | Deadline baseline → candidate | Max RSS baseline → candidate (MiB) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| final / all | 751 | 4584 → 4506 (-78) | 12 / 6 | +2 / 1 | +33 / 15 | 1.000× | 602 → 605 | 27 → 27 | 295.2 → 295.4 |
| final / pcbench | 740 | 4354 → 4277 (-77) | 11 / 6 | +2 / 1 | +33 / 15 | 1.003× | 595 → 597 | 26 → 26 | 295.2 → 295.4 |
| final / local | 11 | 230 → 229 (-1) | 1 / 0 | +0 / 0 | +0 / 0 | 0.875× | 7 → 8 | 1 → 1 | 135.2 → 135.5 |

## Deadline and report-cap controls

- final: both sides completed normally on 723 boards: unrouted Δ -7, copper Δ +0, mask Δ +31. Mask counts below the cap on both sides: 724 boards, Δ -7, 0 gaining findings.

## Per-board changes

| Variant | Board | Unrouted | Copper | Mask | Either side reached deadline? |
|---|---|---:|---:|---:|---|
| final | pcbench-Blitz_.C68 | 499 → 414 | 0 → 0 | 0 → 0 | True |
| final | pcbench-Blitz_copy | 483 → 499 | 0 → 0 | 0 → 0 | True |
| final | pcbench-DoroidOscillo-Board_Android_Oscilloscope | 5 → 4 | 0 → 0 | 48 → 48 | False |
| final | pcbench-Electronics-MainBoard_MainBoard | 0 → 0 | 0 → 0 | 201 → 203 | False |
| final | pcbench-Librecalc-Hardware__autosave-calculator | 1 → 1 | 0 → 0 | 200 → 204 | True |
| final | pcbench-Mechaduino-DR_Mechaduino DR 1.01 | 0 → 0 | 0 → 0 | 203 → 199 | False |
| final | pcbench-MixSID_mixsid | 0 → 0 | 0 → 0 | 202 → 205 | False |
| final | pcbench-NRC2016_banked_ram | 0 → 0 | 0 → 0 | 207 → 200 | False |
| final | pcbench-Own-Mailbox-Hardware_eth | 1 → 1 | 0 → 0 | 204 → 206 | False |
| final | pcbench-Own-Mailbox-Hardware_mailbox | 2 → 2 | 0 → 0 | 203 → 206 | False |
| final | pcbench-ReSDMAC_ReSDMAC | 121 → 119 | 0 → 0 | 0 → 0 | True |
| final | pcbench-Solare-BQ24210_Solare-BQ24210 | 1 → 0 | 0 → 0 | 14 → 14 | False |
| final | pcbench-amalthea_amalthea_rev0 | 23 → 23 | 0 → 0 | 200 → 201 | True |
| final | pcbench-badge2016_Badge_init | 0 → 0 | 0 → 0 | 239 → 256 | False |
| final | pcbench-balena-rover-wide-hat_resin-rover | 0 → 0 | 4 → 4 | 200 → 201 | False |
| final | pcbench-bms-8s50-ic_bms-8s50-ic | 54 → 53 | 0 → 0 | 88 → 88 | True |
| final | pcbench-decelerator4030_decelerator4030 | 474 → 475 | 10 → 12 | 0 → 0 | True |
| final | pcbench-dorkyboard_keyboard | 3 → 3 | 20 → 20 | 204 → 213 | False |
| final | pcbench-gb-hardware_GB-CART256K-A | 14 → 12 | 0 → 0 | 0 → 0 | False |
| final | pcbench-headstage-adapter_headstage adapter | 0 → 0 | 0 → 0 | 204 → 205 | False |
| final | pcbench-karabas-nano_karabas-nano-revA | 29 → 34 | 0 → 0 | 198 → 198 | True |
| final | pcbench-karabas-nano_karabas-nano-revB | 64 → 73 | 0 → 0 | 199 → 199 | True |
| final | pcbench-karabas-nano_karabas-nano-revC | 81 → 77 | 1 → 1 | 200 → 201 | True |
| final | pcbench-karabas-nano_karabas-nano-revG | 85 → 86 | 0 → 0 | 199 → 201 | True |
| final | pcbench-kitspace_40-channel-hv-switching-board | 0 → 0 | 0 → 0 | 202 → 205 | True |
| final | pcbench-kitspace_T32_ref | 0 → 0 | 0 → 0 | 181 → 174 | True |
| final | pcbench-kitspace_dropbot-front-panel | 0 → 0 | 0 → 0 | 208 → 207 | True |
| final | pcbench-kitspace_dropbot_control_board | 0 → 0 | 0 → 0 | 202 → 201 | False |
| final | pcbench-memsarray_mems_array | 0 → 0 | 0 → 0 | 201 → 215 | False |
| final | pcbench-nonSNES_SNSP-CPU-1CHIP | 159 → 162 | 1 → 1 | 28 → 28 | True |
| final | pcbench-pico-pi-rel_pico-pi | 0 → 0 | 0 → 0 | 203 → 200 | False |
| final | pcbench-starfish_starfish | 1 → 0 | 0 → 0 | 0 → 0 | False |
| final | pcbench-timecircuits-hardware_BTTF-TimeCircuits | 0 → 0 | 0 → 0 | 205 → 204 | False |
| final | pcbench-uSKY_uSKY | 24 → 22 | 0 → 0 | 0 → 0 | False |
| final | pcbench-wavegen_rev3 | 12 → 12 | 0 → 0 | 201 → 200 | True |
| final | pcbench-wavegen_waveform-generator-rev1 | 1 → 1 | 0 → 0 | 209 → 201 | False |
| final | pcbench-wavegen_wavegen | 1 → 1 | 0 → 0 | 202 → 205 | False |
| final | pcbench-zx-sizif-512-ext_sizif512ext | 58 → 50 | 0 → 0 | 183 → 183 | True |
| final | pcbench-zx-sizif-xxs_sizif-xxs | 27 → 22 | 0 → 0 | 165 → 165 | True |
| final | kicad-issue269-nowiresonpowerlayers--proba | 1 → 0 | 0 → 0 | 0 → 0 | True |
