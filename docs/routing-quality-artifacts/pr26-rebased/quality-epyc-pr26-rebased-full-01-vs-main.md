# Comparison: main vs neckdown

Created 2026-09-10T06:47:00+00:00 from runs quality-epyc-pr26-rebased-full-01. Config: threads=1 jobs=192 max_passes=10 timeout=300s seeds=1 time metric = cpu_s

| candidate | version | sha |
|---|---|---|
| main | — | `736d02d+main` |
| neckdown | — | `736d02d+neckdown` |

## Overall

- **neckdown** vs main: **worse** — 243 wins, 472 losses (9 on hard metrics), 36 ties

## Coverage

Verdicts summarize comparable shared boards; exclusions are not ties.

| candidate | shared | compared | incomplete | skipped | baseline only | candidate only |
|---|---|---|---|---|---|---|
| neckdown | 751 | 751 | 0 | 0 | 0 | 0 |

## Regression gate

Routing-quality losses fail the default gate. Time/RSS are advisory unless a tolerance is specified.

- neckdown: 24 quality losses, 0 performance losses, 0 boards with insufficient performance samples

## Per tier

| tier | candidate | wins | losses | ties | clean-pass rate | median Δscore | median time ratio |
|---|---|---|---|---|---|---|---|
| all | neckdown | 243 | 472 | 36 | 0.76 | 0.0 | 1.01 |
| d3-a | neckdown | 43 | 92 | 28 | 0.91 | 0.0 | 1.00 |
| d3-b | neckdown | 85 | 229 | 7 | 0.81 | 0.0 | 1.01 |
| d3-c | neckdown | 115 | 151 | 1 | 0.61 | 0.0 | 1.00 |
| kicad-fixtures | neckdown | 1 | 9 | 1 | 0.55 | 0.0 | 1.02 |
| pcbench | neckdown | 242 | 463 | 35 | 0.76 | 0.0 | 1.01 |

## Per board

| board | referee | candidate | clean | unrouted | viol | score | Δscore | noise | cpu s | Δcpu s | vias | length mm | verdict |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| kicad-issue180-test--test | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 4.4 | | 0 | 659.4 | baseline |
| kicad-issue180-test--test | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 4.7 | +0.3 | 0 | 659.4 | loss (cpu_s) |
| kicad-issue184-motorizedopener--motorizedopener | kicad | main | 0.00 | 61 | 54 | 79.5 | | unmeasured | 187.7 | | 28 | 733.5 | baseline |
| kicad-issue184-motorizedopener--motorizedopener | kicad | neckdown | 0.00 | 54 | 61 | 151.3 | +71.8 | | 189.8 | +2.1 | 44 | 795.3 | win (unrouted) |
| kicad-issue269-min_fr_test--min_fr_test | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.1 | | 0 | 27.3 | baseline |
| kicad-issue269-min_fr_test--min_fr_test | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.1 | +0.0 | 0 | 27.3 | loss (cpu_s) |
| kicad-issue269-noviasonpowerplanes--issue269-noviasonpowerplanes | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.5 | | 2 | 234.1 | baseline |
| kicad-issue269-noviasonpowerplanes--issue269-noviasonpowerplanes | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.7 | +0.2 | 2 | 234.1 | loss (cpu_s) |
| kicad-issue269-nowiresonpowerlayers--proba | kicad | main | 0.00 | 1 | 0 | 995.5 | | unmeasured | 298.7 | | 333 | 6633.9 | baseline |
| kicad-issue269-nowiresonpowerlayers--proba | kicad | neckdown | 0.00 | 1 | 0 | 995.5 | -0.0 | | 298.8 | +0.1 | 333 | 6635.1 | loss (score) |
| kicad-issue283-unconnectedtracesunderpads--natural_tone_preamp | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 208.3 | | 113 | 2670.4 | baseline |
| kicad-issue283-unconnectedtracesunderpads--natural_tone_preamp | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 210.1 | +1.8 | 113 | 2670.4 | loss (cpu_s) |
| kicad-issue367-ultraflactyl--ultraflactyl | kicad | main | 0.00 | 1 | 0 | 986.7 | | unmeasured | 73.4 | | 16 | 2705.5 | baseline |
| kicad-issue367-ultraflactyl--ultraflactyl | kicad | neckdown | 0.00 | 1 | 0 | 986.7 | 0.0 | | 74.9 | +1.4 | 16 | 2705.5 | loss (cpu_s) |
| kicad-issue368-corneyislandwireless--corney_island_wireless | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.0 | | 0 | 25.7 | baseline |
| kicad-issue368-corneyislandwireless--corney_island_wireless | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.0 | 0.0 | 0 | 25.7 | tie (peak_rss_mb) |
| kicad-issue558-dev-board-autoroute-demo--dev-board | kicad | main | 0.00 | 0 | 4 | 983.0 | | unmeasured | 10.0 | | 22 | 1107.4 | baseline |
| kicad-issue558-dev-board-autoroute-demo--dev-board | kicad | neckdown | 0.00 | 0 | 4 | 983.0 | 0.0 | | 10.2 | +0.2 | 22 | 1107.4 | loss (cpu_s) |
| kicad-issue632-miniautopilot--mini auto pilot | kicad | main | 0.00 | 167 | 0 | 208.5 | | unmeasured | 3.0 | | 0 | 0.0 | baseline |
| kicad-issue632-miniautopilot--mini auto pilot | kicad | neckdown | 0.00 | 167 | 0 | 208.5 | 0.0 | | 3.3 | +0.3 | 0 | 0.0 | loss (cpu_s) |
| kicad-issue742-tastexx-pcb--tastexx-pcb | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.0 | | 3 | 534.8 | baseline |
| kicad-issue742-tastexx-pcb--tastexx-pcb | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.2 | +0.2 | 3 | 534.8 | loss (cpu_s) |
| pcbench-1-Wire-Wing-pcb_1-Wire_Wing | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 4.5 | | 7 | 451.6 | baseline |
| pcbench-1-Wire-Wing-pcb_1-Wire_Wing | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 4.8 | +0.3 | 7 | 451.6 | loss (cpu_s) |
| pcbench-12v-automatic-ups_ups-12v | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 11.4 | | 0 | 1994.8 | baseline |
| pcbench-12v-automatic-ups_ups-12v | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 11.9 | +0.5 | 0 | 1994.8 | loss (cpu_s) |
| pcbench-16x12-bits-I2C_I2C_Servo | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 7.6 | | 19 | 1080.5 | baseline |
| pcbench-16x12-bits-I2C_I2C_Servo | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 8.0 | +0.4 | 19 | 1080.5 | loss (cpu_s) |
| pcbench-1Bitsy_1bitsy | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 173.4 | | 67 | 1084.1 | baseline |
| pcbench-1Bitsy_1bitsy | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 176.0 | +2.6 | 67 | 1084.1 | loss (cpu_s) |
| pcbench-2d_conduction_sk9822-matrix | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 135.6 | | 8 | 3056.9 | baseline |
| pcbench-2d_conduction_sk9822-matrix | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 137.1 | +1.5 | 8 | 3056.9 | loss (cpu_s) |
| pcbench-4N35-TTL-Serial-Optoisolator_4N35-TTL-Serial-Optoisolator | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.1 | | 0 | 215.9 | baseline |
| pcbench-4N35-TTL-Serial-Optoisolator_4N35-TTL-Serial-Optoisolator | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.2 | +0.1 | 0 | 215.9 | loss (cpu_s) |
| pcbench-655_testboard | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 21.8 | | 23 | 959.4 | baseline |
| pcbench-655_testboard | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 12.4 | -9.4 | 22 | 969.5 | win (score) |
| pcbench-6N137-TTL-Serial-Optoisolator_6N137-TTL-Serial-Optoisolator | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.2 | | 0 | 286.4 | baseline |
| pcbench-6N137-TTL-Serial-Optoisolator_6N137-TTL-Serial-Optoisolator | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.4 | +0.1 | 0 | 286.4 | loss (cpu_s) |
| pcbench-6volt-5W-solar-cc_6vleadacidsolar | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 17.5 | | 8 | 910.1 | baseline |
| pcbench-6volt-5W-solar-cc_6vleadacidsolar | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 17.8 | +0.3 | 8 | 910.1 | loss (cpu_s) |
| pcbench-74Logic_SA_ADC_SA-ADC | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 237.9 | | 144 | 4654.3 | baseline |
| pcbench-74Logic_SA_ADC_SA-ADC | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 236.2 | -1.7 | 144 | 4654.3 | win (cpu_s) |
| pcbench-8bit-cpu_arduino_eeprom_programmer | kicad | main | 0.00 | 2 | 0 | 939.4 | | unmeasured | 21.4 | | 9 | 1240.8 | baseline |
| pcbench-8bit-cpu_arduino_eeprom_programmer | kicad | neckdown | 0.00 | 2 | 0 | 939.4 | 0.0 | | 21.8 | +0.4 | 9 | 1240.8 | loss (cpu_s) |
| pcbench-8bit-cpu_programming_interface | kicad | main | 0.00 | 5 | 0 | 861.1 | | unmeasured | 78.3 | | 15 | 1408.5 | baseline |
| pcbench-8bit-cpu_programming_interface | kicad | neckdown | 0.00 | 5 | 0 | 861.1 | 0.0 | | 79.0 | +0.7 | 15 | 1408.5 | loss (cpu_s) |
| pcbench-96boards-sensors_Sensors | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 258.1 | | 163 | 3951.3 | baseline |
| pcbench-96boards-sensors_Sensors | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 250.1 | -7.9 | 163 | 3951.3 | win (cpu_s) |
| pcbench-ABOVISP_ABOVISP | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.5 | | 2 | 204.2 | baseline |
| pcbench-ABOVISP_ABOVISP | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.6 | +0.1 | 2 | 204.2 | loss (cpu_s) |
| pcbench-ADC-PCM4202-SE_ADC-PCM4202-SE | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 78.4 | | 32 | 2702.4 | baseline |
| pcbench-ADC-PCM4202-SE_ADC-PCM4202-SE | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 78.3 | -0.1 | 32 | 2702.4 | win (cpu_s) |
| pcbench-AIOsense_AIOsense | kicad | main | 0.00 | 18 | 0 | 600.0 | | unmeasured | 96.0 | | 0 | 807.4 | baseline |
| pcbench-AIOsense_AIOsense | kicad | neckdown | 0.00 | 18 | 0 | 600.0 | 0.0 | | 97.2 | +1.3 | 0 | 807.4 | loss (cpu_s) |
| pcbench-APC_AtariPunkConsole | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.2 | | 0 | 453.4 | baseline |
| pcbench-APC_AtariPunkConsole | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.4 | +0.2 | 0 | 453.4 | loss (cpu_s) |
| pcbench-APM-RPi-Shield_APM-RPi-Shield | kicad | main | 0.00 | 0 | 1 | 996.8 | | unmeasured | 61.2 | | 19 | 1272.5 | baseline |
| pcbench-APM-RPi-Shield_APM-RPi-Shield | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | +3.2 | | 61.8 | +0.6 | 19 | 1270.4 | win (clean_pass_rate) |
| pcbench-AS5043-Encoder_sensor-board | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.9 | | 4 | 258.1 | baseline |
| pcbench-AS5043-Encoder_sensor-board | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.0 | +0.1 | 4 | 258.1 | loss (cpu_s) |
| pcbench-ATmega32_ExploreUltraAvrDevKit_40pin_AVRMCU | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 10.3 | | 0 | 1147.7 | baseline |
| pcbench-ATmega32_ExploreUltraAvrDevKit_40pin_AVRMCU | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 10.7 | +0.5 | 0 | 1147.7 | loss (cpu_s) |
| pcbench-ATtiny461Breakout_ATTiny461DevBoard | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.3 | | 1 | 325.9 | baseline |
| pcbench-ATtiny461Breakout_ATTiny461DevBoard | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.4 | +0.2 | 1 | 325.9 | loss (cpu_s) |
| pcbench-AVR-ISP_level-shifter_AVR-ISP_level-shifter | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 3.7 | | 0 | 418.9 | baseline |
| pcbench-AVR-ISP_level-shifter_AVR-ISP_level-shifter | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 3.8 | +0.2 | 0 | 418.9 | loss (cpu_s) |
| pcbench-AVR-ISP_pogo-plug_1.27mm_AVR-ISP_pogo-plug_1.27mm | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.3 | | 0 | 64.5 | baseline |
| pcbench-AVR-ISP_pogo-plug_1.27mm_AVR-ISP_pogo-plug_1.27mm | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.4 | +0.1 | 0 | 64.5 | loss (cpu_s) |
| pcbench-AVR-Playground_hello_world | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.1 | | 0 | 135.6 | baseline |
| pcbench-AVR-Playground_hello_world | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.1 | +0.0 | 0 | 135.6 | loss (cpu_s) |
| pcbench-AVR-ZIF-Programmer_AVR-ZIF-Prog | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 15.2 | | 0 | 1464.3 | baseline |
| pcbench-AVR-ZIF-Programmer_AVR-ZIF-Prog | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 15.6 | +0.4 | 0 | 1464.3 | loss (cpu_s) |
| pcbench-Amiga-2000-EATX_TICK_OSC_KiCAD_TICK | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.8 | | 4 | 126.5 | baseline |
| pcbench-Amiga-2000-EATX_TICK_OSC_KiCAD_TICK | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.9 | +0.2 | 4 | 126.5 | loss (cpu_s) |
| pcbench-Amiga-A1012-PCB_Amiga-A1012 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 31.5 | | 33 | 1779.1 | baseline |
| pcbench-Amiga-A1012-PCB_Amiga-A1012 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 32.5 | +1.0 | 33 | 1779.1 | loss (cpu_s) |
| pcbench-AmpOne_dev-AmpOne | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 128.9 | | 26 | 2826.8 | baseline |
| pcbench-AmpOne_dev-AmpOne | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 130.2 | +1.3 | 26 | 2826.8 | loss (cpu_s) |
| pcbench-AnalogThermometer_AnalogThermometer | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.7 | | 4 | 206.4 | baseline |
| pcbench-AnalogThermometer_AnalogThermometer | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.8 | +0.1 | 4 | 206.4 | loss (cpu_s) |
| pcbench-Apple-M0110-BT_Apple M0110 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 7.0 | | 2 | 4367.6 | baseline |
| pcbench-Apple-M0110-BT_Apple M0110 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 7.3 | +0.3 | 2 | 4367.6 | loss (cpu_s) |
| pcbench-Arduino-Theremin_arduino-theremin-v1 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.3 | | 1 | 634.6 | baseline |
| pcbench-Arduino-Theremin_arduino-theremin-v1 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.4 | +0.1 | 1 | 634.6 | loss (cpu_s) |
| pcbench-Arduino_Lipo_Storage_Discharger_Lipo_Storage_Discharger | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 9.1 | | 2 | 1052.4 | baseline |
| pcbench-Arduino_Lipo_Storage_Discharger_Lipo_Storage_Discharger | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 9.4 | +0.2 | 2 | 1052.4 | loss (cpu_s) |
| pcbench-Atmel-ICE-Header-Adapter_ice header adapter pcb | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 19.6 | | 6 | 883.3 | baseline |
| pcbench-Atmel-ICE-Header-Adapter_ice header adapter pcb | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 19.8 | +0.2 | 6 | 883.3 | loss (cpu_s) |
| pcbench-Avem_Hardware_Avem_demo | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 36.2 | | 12 | 714.3 | baseline |
| pcbench-Avem_Hardware_Avem_demo | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 21.8 | -14.5 | 12 | 721.6 | loss (score) |
| pcbench-AzizLight_AzizLight | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 31.4 | | 7 | 769.8 | baseline |
| pcbench-AzizLight_AzizLight | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 32.0 | +0.6 | 7 | 769.8 | loss (cpu_s) |
| pcbench-BB-PWR-3608_BB-PWR-3608_revA | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.5 | | 3 | 78.3 | baseline |
| pcbench-BB-PWR-3608_BB-PWR-3608_revA | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.6 | +0.1 | 3 | 78.3 | loss (cpu_s) |
| pcbench-BB-PWR-8009_BB-PWR-8009_revA | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.4 | | 2 | 72.2 | baseline |
| pcbench-BB-PWR-8009_BB-PWR-8009_revA | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.5 | +0.2 | 2 | 72.2 | loss (cpu_s) |
| pcbench-BLDC-controller_BLDC_controller | kicad | main | 0.00 | 12 | 9 | 784.4 | | unmeasured | 280.1 | | 50 | 1378.5 | baseline |
| pcbench-BLDC-controller_BLDC_controller | kicad | neckdown | 0.00 | 11 | 13 | 787.5 | +3.1 | | 181.2 | -98.9 | 54 | 1406.6 | win (unrouted) |
| pcbench-BML-Badges_BML-Badges | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.3 | | 0 | 469.3 | baseline |
| pcbench-BML-Badges_BML-Badges | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.4 | +0.1 | 0 | 469.3 | loss (cpu_s) |
| pcbench-BML-Badges_BML_01 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.7 | | 0 | 334.9 | baseline |
| pcbench-BML-Badges_BML_01 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.8 | +0.1 | 0 | 334.9 | loss (cpu_s) |
| pcbench-Baofeng-Interface_BaofengInterfaceIsolated | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 29.2 | | 0 | 645.9 | baseline |
| pcbench-Baofeng-Interface_BaofengInterfaceIsolated | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 30.1 | +0.9 | 0 | 645.9 | loss (cpu_s) |
| pcbench-BirdAttractor_BirdAttractor_RevA | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.7 | | 2 | 364.8 | baseline |
| pcbench-BirdAttractor_BirdAttractor_RevA | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.9 | +0.1 | 2 | 364.8 | loss (cpu_s) |
| pcbench-BirdAttractor_BirdAttractor_RevC | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 25.4 | | 24 | 1152.2 | baseline |
| pcbench-BirdAttractor_BirdAttractor_RevC | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 26.5 | +1.1 | 24 | 1152.2 | loss (cpu_s) |
| pcbench-BirthdayCakeKeyboard_10Key | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 197.9 | | 61 | 3824.5 | baseline |
| pcbench-BirthdayCakeKeyboard_10Key | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 200.3 | +2.4 | 61 | 3824.5 | loss (cpu_s) |
| pcbench-Biscay_Blueeye_mcu | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 8.0 | | 0 | 1234.6 | baseline |
| pcbench-Biscay_Blueeye_mcu | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 8.2 | +0.1 | 0 | 1234.6 | loss (cpu_s) |
| pcbench-Biscay_Blueeye_sipm-fpga | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 68.0 | | 19 | 1476.4 | baseline |
| pcbench-Biscay_Blueeye_sipm-fpga | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 69.1 | +1.1 | 19 | 1476.4 | loss (cpu_s) |
| pcbench-Blink-Eras_AVR_ISP_Pogo | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.0 | | 0 | 97.3 | baseline |
| pcbench-Blink-Eras_AVR_ISP_Pogo | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.1 | +0.0 | 0 | 97.3 | loss (cpu_s) |
| pcbench-Blitz_.C68 | kicad | main | 0.00 | 499 | 0 | 0.0 | | unmeasured | 302.1 | | 673 | 7335.2 | baseline |
| pcbench-Blitz_.C68 | kicad | neckdown | 0.00 | 499 | 0 | 0.0 | 0.0 | | 301.6 | -0.4 | 674 | 8967.1 | win (cpu_s) |
| pcbench-Blitz_Rev.K_C68 | kicad | main | 0.00 | 499 | 0 | 0.0 | | unmeasured | 301.6 | | 667 | 6863.1 | baseline |
| pcbench-Blitz_Rev.K_C68 | kicad | neckdown | 0.00 | 499 | 0 | 0.0 | 0.0 | | 301.2 | -0.4 | 668 | 9487.1 | win (cpu_s) |
| pcbench-Blitz_copy | kicad | main | 0.00 | 499 | 0 | 0.0 | | unmeasured | 301.8 | | 667 | 6851.9 | baseline |
| pcbench-Blitz_copy | kicad | neckdown | 0.00 | 499 | 0 | 0.0 | 0.0 | | 301.6 | -0.2 | 671 | 9835.9 | win (cpu_s) |
| pcbench-Box0-hv-analog-breakoutboard_breakout | kicad | main | 0.00 | 128 | 0 | 0.0 | | unmeasured | 60.7 | | 0 | 101.2 | baseline |
| pcbench-Box0-hv-analog-breakoutboard_breakout | kicad | neckdown | 0.00 | 128 | 0 | 0.0 | 0.0 | | 61.0 | +0.3 | 0 | 101.2 | loss (cpu_s) |
| pcbench-Brushless_ESC_Brushless_ESC | kicad | main | 0.00 | 0 | 1 | 996.0 | | unmeasured | 58.5 | | 31 | 1433.8 | baseline |
| pcbench-Brushless_ESC_Brushless_ESC | kicad | neckdown | 0.00 | 0 | 1 | 996.0 | 0.0 | | 58.9 | +0.5 | 31 | 1433.8 | loss (cpu_s) |
| pcbench-C-BISCUIT_crowbar | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.2 | | 2 | 126.1 | baseline |
| pcbench-C-BISCUIT_crowbar | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.2 | +0.0 | 2 | 126.1 | loss (cpu_s) |
| pcbench-CAL430FR_CAL430F | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 198.6 | | 68 | 1259.3 | baseline |
| pcbench-CAL430FR_CAL430F | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 199.2 | +0.6 | 68 | 1259.3 | loss (cpu_s) |
| pcbench-CAL430FR_CAL430F_watch | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.3 | | 1 | 364.7 | baseline |
| pcbench-CAL430FR_CAL430F_watch | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.3 | -0.0 | 1 | 364.7 | win (cpu_s) |
| pcbench-CATs-Eurosynth_4CH_Mixer | kicad | main | 0.00 | 2 | 0 | 894.7 | | unmeasured | 3.5 | | 1 | 423.0 | baseline |
| pcbench-CATs-Eurosynth_4CH_Mixer | kicad | neckdown | 0.00 | 2 | 0 | 894.7 | 0.0 | | 3.6 | +0.2 | 1 | 423.0 | loss (cpu_s) |
| pcbench-CATs-Eurosynth_APC_Eurorack_Main | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.5 | | 0 | 287.6 | baseline |
| pcbench-CATs-Eurosynth_APC_Eurorack_Main | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.6 | +0.1 | 0 | 287.6 | loss (cpu_s) |
| pcbench-CATs-Eurosynth_Abakus_Control | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 5.7 | | 0 | 1405.9 | baseline |
| pcbench-CATs-Eurosynth_Abakus_Control | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 5.9 | +0.2 | 0 | 1405.9 | loss (cpu_s) |
| pcbench-CATs-Eurosynth_Abakus_Main | kicad | main | 0.00 | 15 | 0 | 750.0 | | unmeasured | 130.1 | | 70 | 2727.6 | baseline |
| pcbench-CATs-Eurosynth_Abakus_Main | kicad | neckdown | 0.00 | 15 | 0 | 750.0 | 0.0 | | 131.3 | +1.2 | 70 | 2727.6 | loss (cpu_s) |
| pcbench-CATs-Eurosynth_Arduino_VCO_Main | kicad | main | 0.00 | 7 | 0 | 720.0 | | unmeasured | 11.0 | | 6 | 678.9 | baseline |
| pcbench-CATs-Eurosynth_Arduino_VCO_Main | kicad | neckdown | 0.00 | 7 | 0 | 720.0 | 0.0 | | 11.4 | +0.3 | 6 | 678.9 | loss (cpu_s) |
| pcbench-CATs-Eurosynth_Baby_8 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.5 | | 0 | 954.2 | baseline |
| pcbench-CATs-Eurosynth_Baby_8 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.5 | +0.1 | 0 | 954.2 | loss (cpu_s) |
| pcbench-CATs-Eurosynth_Buffered_Multiple_Main | kicad | main | 0.00 | 1 | 0 | 947.4 | | unmeasured | 3.4 | | 0 | 544.3 | baseline |
| pcbench-CATs-Eurosynth_Buffered_Multiple_Main | kicad | neckdown | 0.00 | 1 | 0 | 947.4 | 0.0 | | 3.5 | +0.1 | 0 | 544.3 | loss (cpu_s) |
| pcbench-CATs-Eurosynth_Buffered_Multiple_SMD_Main | kicad | main | 0.00 | 2 | 6 | 889.6 | | unmeasured | 10.6 | | 9 | 687.2 | baseline |
| pcbench-CATs-Eurosynth_Buffered_Multiple_SMD_Main | kicad | neckdown | 0.00 | 2 | 6 | 889.6 | 0.0 | | 10.8 | +0.2 | 9 | 687.2 | loss (cpu_s) |
| pcbench-CATs-Eurosynth_CGS_Analog_Switch_Matrix_Control | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 3.7 | | 0 | 431.9 | baseline |
| pcbench-CATs-Eurosynth_CGS_Analog_Switch_Matrix_Control | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 3.7 | +0.0 | 0 | 431.9 | loss (cpu_s) |
| pcbench-CATs-Eurosynth_CGS_Analog_Switch_Matrix_Main | kicad | main | 0.00 | 8 | 0 | 846.1 | | unmeasured | 88.4 | | 34 | 1458.1 | baseline |
| pcbench-CATs-Eurosynth_CGS_Analog_Switch_Matrix_Main | kicad | neckdown | 0.00 | 8 | 0 | 846.1 | 0.0 | | 90.5 | +2.1 | 34 | 1458.1 | loss (cpu_s) |
| pcbench-CATs-Eurosynth_Classic_ADSR_Main | kicad | main | 0.00 | 3 | 0 | 918.9 | | unmeasured | 37.2 | | 24 | 947.2 | baseline |
| pcbench-CATs-Eurosynth_Classic_ADSR_Main | kicad | neckdown | 0.00 | 3 | 0 | 918.9 | 0.0 | | 37.7 | +0.5 | 24 | 947.2 | loss (cpu_s) |
| pcbench-CATs-Eurosynth_Clock_Divider_Control | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.4 | | 0 | 363.8 | baseline |
| pcbench-CATs-Eurosynth_Clock_Divider_Control | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.5 | +0.1 | 0 | 363.8 | loss (cpu_s) |
| pcbench-CATs-Eurosynth_Clock_Divider_Main | kicad | main | 0.00 | 1 | 0 | 967.7 | | unmeasured | 5.7 | | 0 | 637.2 | baseline |
| pcbench-CATs-Eurosynth_Clock_Divider_Main | kicad | neckdown | 0.00 | 1 | 0 | 967.7 | 0.0 | | 5.7 | +0.0 | 0 | 637.2 | loss (cpu_s) |
| pcbench-CATs-Eurosynth_Dual_VCA_2_Control | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.4 | | 0 | 390.6 | baseline |
| pcbench-CATs-Eurosynth_Dual_VCA_2_Control | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.6 | +0.2 | 0 | 390.6 | loss (cpu_s) |
| pcbench-CATs-Eurosynth_Dual_VCA_2_Main | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 11.8 | | 0 | 870.2 | baseline |
| pcbench-CATs-Eurosynth_Dual_VCA_2_Main | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 12.1 | +0.3 | 0 | 870.2 | loss (cpu_s) |
| pcbench-CATs-Eurosynth_Envelope_Follower_Main | kicad | main | 0.00 | 0 | 4 | 960.0 | | unmeasured | 3.3 | | 0 | 558.7 | baseline |
| pcbench-CATs-Eurosynth_Envelope_Follower_Main | kicad | neckdown | 0.00 | 0 | 4 | 960.0 | 0.0 | | 3.4 | +0.1 | 0 | 558.7 | loss (cpu_s) |
| pcbench-CATs-Eurosynth_HAGIWO_6Ch_Gate_Sequencer_Main | kicad | main | 0.00 | 1 | 0 | 958.3 | | unmeasured | 4.7 | | 3 | 788.5 | baseline |
| pcbench-CATs-Eurosynth_HAGIWO_6Ch_Gate_Sequencer_Main | kicad | neckdown | 0.00 | 1 | 0 | 958.3 | 0.0 | | 4.9 | +0.2 | 3 | 788.5 | loss (cpu_s) |
| pcbench-CATs-Eurosynth_HAGIWO_MultiOut_Control | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.7 | | 0 | 244.4 | baseline |
| pcbench-CATs-Eurosynth_HAGIWO_MultiOut_Control | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.8 | +0.1 | 0 | 244.4 | loss (cpu_s) |
| pcbench-CATs-Eurosynth_HAGIWO_MultiOut_Main | kicad | main | 0.00 | 7 | 0 | 810.8 | | unmeasured | 18.5 | | 9 | 1087.0 | baseline |
| pcbench-CATs-Eurosynth_HAGIWO_MultiOut_Main | kicad | neckdown | 0.00 | 7 | 0 | 810.8 | 0.0 | | 19.0 | +0.5 | 9 | 1087.0 | loss (cpu_s) |
| pcbench-CATs-Eurosynth_HAGIWO_Ring_Modulator_Main | kicad | main | 0.00 | 4 | 0 | 885.7 | | unmeasured | 13.3 | | 10 | 692.2 | baseline |
| pcbench-CATs-Eurosynth_HAGIWO_Ring_Modulator_Main | kicad | neckdown | 0.00 | 4 | 0 | 885.7 | 0.0 | | 13.3 | +0.0 | 10 | 692.2 | loss (cpu_s) |
| pcbench-CATs-Eurosynth_LFO_Main | kicad | main | 0.00 | 3 | 0 | 903.2 | | unmeasured | 28.0 | | 20 | 881.3 | baseline |
| pcbench-CATs-Eurosynth_LFO_Main | kicad | neckdown | 0.00 | 3 | 0 | 903.2 | 0.0 | | 28.2 | +0.2 | 20 | 881.3 | loss (cpu_s) |
| pcbench-CATs-Eurosynth_MFOS_Eight_Stage_Phase_Shifter_-_Control | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.7 | | 0 | 393.2 | baseline |
| pcbench-CATs-Eurosynth_MFOS_Eight_Stage_Phase_Shifter_-_Control | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.8 | +0.1 | 0 | 393.2 | loss (cpu_s) |
| pcbench-CATs-Eurosynth_MFOS_Eight_Stage_Phase_Shifter_-_Main | kicad | main | 0.00 | 20 | 0 | 677.4 | | unmeasured | 94.5 | | 34 | 1419.6 | baseline |
| pcbench-CATs-Eurosynth_MFOS_Eight_Stage_Phase_Shifter_-_Main | kicad | neckdown | 0.00 | 20 | 0 | 677.4 | 0.0 | | 95.5 | +1.0 | 34 | 1419.6 | loss (cpu_s) |
| pcbench-CATs-Eurosynth_MFOS_Noise_Cornucopia_Control | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.7 | | 0 | 421.3 | baseline |
| pcbench-CATs-Eurosynth_MFOS_Noise_Cornucopia_Control | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.8 | +0.1 | 0 | 421.3 | loss (cpu_s) |
| pcbench-CATs-Eurosynth_MFOS_Noise_Cornucopia_Main_1 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 4.2 | | 0 | 449.6 | baseline |
| pcbench-CATs-Eurosynth_MFOS_Noise_Cornucopia_Main_1 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 4.4 | +0.2 | 0 | 449.6 | loss (cpu_s) |
| pcbench-CATs-Eurosynth_MFOS_Noise_Cornucopia_Main_2 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 5.8 | | 0 | 605.2 | baseline |
| pcbench-CATs-Eurosynth_MFOS_Noise_Cornucopia_Main_2 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 5.8 | +0.1 | 0 | 605.2 | loss (cpu_s) |
| pcbench-CATs-Eurosynth_Main_Rectifier_Main | kicad | main | 0.00 | 0 | 3 | 972.7 | | unmeasured | 13.6 | | 7 | 534.4 | baseline |
| pcbench-CATs-Eurosynth_Main_Rectifier_Main | kicad | neckdown | 0.00 | 0 | 3 | 972.7 | 0.0 | | 14.1 | +0.4 | 7 | 534.4 | loss (cpu_s) |
| pcbench-CATs-Eurosynth_Manual_Gate_Control | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.5 | | 0 | 304.9 | baseline |
| pcbench-CATs-Eurosynth_Manual_Gate_Control | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.7 | +0.2 | 0 | 304.9 | loss (cpu_s) |
| pcbench-CATs-Eurosynth_Manual_Gate_Main | kicad | main | 0.00 | 5 | 0 | 861.1 | | unmeasured | 6.3 | | 0 | 558.0 | baseline |
| pcbench-CATs-Eurosynth_Manual_Gate_Main | kicad | neckdown | 0.00 | 5 | 0 | 861.1 | 0.0 | | 6.7 | +0.4 | 0 | 558.0 | loss (cpu_s) |
| pcbench-CATs-Eurosynth_Power_Modul_Main | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.1 | | 0 | 31.7 | baseline |
| pcbench-CATs-Eurosynth_Power_Modul_Main | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.1 | +0.0 | 0 | 31.7 | loss (cpu_s) |
| pcbench-CATs-Eurosynth_S_H_Main | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.7 | | 0 | 564.2 | baseline |
| pcbench-CATs-Eurosynth_S_H_Main | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.8 | +0.1 | 0 | 564.2 | loss (cpu_s) |
| pcbench-CATs-Eurosynth_Single_Attenuator | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.1 | | 0 | 101.8 | baseline |
| pcbench-CATs-Eurosynth_Single_Attenuator | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.1 | +0.0 | 0 | 101.8 | loss (cpu_s) |
| pcbench-CATs-Eurosynth_Slim_Precision_Adder_Main | kicad | main | 0.00 | 4 | 0 | 862.1 | | unmeasured | 19.8 | | 12 | 852.6 | baseline |
| pcbench-CATs-Eurosynth_Slim_Precision_Adder_Main | kicad | neckdown | 0.00 | 4 | 0 | 862.1 | 0.0 | | 19.8 | 0.0 | 12 | 852.6 | loss (peak_rss_mb) |
| pcbench-CATs-Eurosynth_Slimline_VCA_Main | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 10.1 | | 0 | 670.9 | baseline |
| pcbench-CATs-Eurosynth_Slimline_VCA_Main | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 10.3 | +0.3 | 0 | 670.9 | loss (cpu_s) |
| pcbench-CATs-Eurosynth_Slimline_Voltage_Controlled_Switch_-_Main | kicad | main | 0.00 | 5 | 0 | 857.1 | | unmeasured | 33.1 | | 15 | 987.4 | baseline |
| pcbench-CATs-Eurosynth_Slimline_Voltage_Controlled_Switch_-_Main | kicad | neckdown | 0.00 | 5 | 0 | 857.1 | 0.0 | | 33.7 | +0.7 | 15 | 987.4 | loss (cpu_s) |
| pcbench-CATs-Eurosynth_Vactrol_VCF_-_Main | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 5.6 | | 0 | 713.4 | baseline |
| pcbench-CATs-Eurosynth_Vactrol_VCF_-_Main | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 5.8 | +0.2 | 0 | 713.4 | loss (cpu_s) |
| pcbench-CATs-Eurosynth_YuSynth_Dual_Balanced_Modulator_Main | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 8.7 | | 1 | 600.1 | baseline |
| pcbench-CATs-Eurosynth_YuSynth_Dual_Balanced_Modulator_Main | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 9.0 | +0.3 | 1 | 600.1 | loss (cpu_s) |
| pcbench-CATs-Eurosynth_YuSynth_Improved_Steiner_VCF_Main | kicad | main | 0.00 | 3 | 0 | 940.0 | | unmeasured | 23.1 | | 9 | 1075.3 | baseline |
| pcbench-CATs-Eurosynth_YuSynth_Improved_Steiner_VCF_Main | kicad | neckdown | 0.00 | 3 | 0 | 940.0 | 0.0 | | 23.1 | -0.1 | 9 | 1075.3 | win (cpu_s) |
| pcbench-CapPCB_CapPcb | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.1 | | 0 | 77.3 | baseline |
| pcbench-CapPCB_CapPcb | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.1 | 0.0 | 0 | 77.3 | tie (peak_rss_mb) |
| pcbench-Cherry-Mx-Bitboard_Cherry Mx Bitboard | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.1 | | 0 | 72.5 | baseline |
| pcbench-Cherry-Mx-Bitboard_Cherry Mx Bitboard | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.1 | 0.0 | 0 | 72.5 | tie (peak_rss_mb) |
| pcbench-ChirpHardware_chirp | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 38.7 | | 16 | 808.9 | baseline |
| pcbench-ChirpHardware_chirp | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 39.0 | +0.3 | 16 | 808.9 | loss (cpu_s) |
| pcbench-CompactFlashBreakout_CompactFlashBreakout | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 23.4 | | 16 | 461.0 | baseline |
| pcbench-CompactFlashBreakout_CompactFlashBreakout | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 24.1 | +0.6 | 16 | 461.0 | loss (cpu_s) |
| pcbench-CoreOne-xCORE200-Original_CoreOne | kicad | main | 0.00 | 4 | 0 | 983.7 | | unmeasured | 299.9 | | 301 | 7446.7 | baseline |
| pcbench-CoreOne-xCORE200-Original_CoreOne | kicad | neckdown | 0.00 | 4 | 0 | 983.7 | -0.0 | | 300.3 | +0.4 | 304 | 7438.7 | loss (score) |
| pcbench-CubeSAT-Reaction-Wheel_Edison_Motor_Servo | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.1 | | 0 | 115.7 | baseline |
| pcbench-CubeSAT-Reaction-Wheel_Edison_Motor_Servo | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.1 | 0.0 | 0 | 115.7 | tie (peak_rss_mb) |
| pcbench-CubeSAT-Reaction-Wheel__autosave-GPIO to motor | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.9 | | 0 | 664.8 | baseline |
| pcbench-CubeSAT-Reaction-Wheel__autosave-GPIO to motor | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.9 | -0.0 | 0 | 664.8 | win (cpu_s) |
| pcbench-Curryboard_Curryboard | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 166.0 | | 64 | 1960.7 | baseline |
| pcbench-Curryboard_Curryboard | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 168.6 | +2.6 | 64 | 1960.7 | loss (cpu_s) |
| pcbench-DAC-ADAU1966_DAC-ADAU1966 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 252.2 | | 82 | 3970.9 | baseline |
| pcbench-DAC-ADAU1966_DAC-ADAU1966 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 243.3 | -8.9 | 82 | 3970.9 | win (cpu_s) |
| pcbench-DC25_DC25 | kicad | main | 0.00 | 0 | 11 | 943.6 | | unmeasured | 27.6 | | 19 | 1573.8 | baseline |
| pcbench-DC25_DC25 | kicad | neckdown | 0.00 | 0 | 11 | 943.6 | 0.0 | | 28.5 | +0.9 | 19 | 1573.8 | loss (cpu_s) |
| pcbench-DIYDAC_DIYDAC | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.1 | | 0 | 68.1 | baseline |
| pcbench-DIYDAC_DIYDAC | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.1 | 0.0 | 0 | 68.1 | win (peak_rss_mb) |
| pcbench-DPS-1200FB_Adapter_Adapter | kicad | main | 0.00 | 0 | 2 | 955.5 | | unmeasured | 11.1 | | 0 | 799.9 | baseline |
| pcbench-DPS-1200FB_Adapter_Adapter | kicad | neckdown | 0.00 | 0 | 2 | 955.5 | 0.0 | | 11.1 | +0.0 | 0 | 799.9 | loss (cpu_s) |
| pcbench-DaWeather---Project__autosave-CarteDaWeather | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.1 | | 0 | 580.6 | baseline |
| pcbench-DaWeather---Project__autosave-CarteDaWeather | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.2 | +0.0 | 0 | 580.6 | loss (cpu_s) |
| pcbench-DasBlinkinput_Das Blinkinput | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 10.6 | | 15 | 624.8 | baseline |
| pcbench-DasBlinkinput_Das Blinkinput | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 10.3 | -0.3 | 15 | 624.8 | win (cpu_s) |
| pcbench-Dekada_dekada | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.3 | | 0 | 379.6 | baseline |
| pcbench-Dekada_dekada | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.3 | 0.0 | 0 | 379.6 | tie (peak_rss_mb) |
| pcbench-Dekada_dekada_TopoR_curves | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.3 | | 0 | 379.6 | baseline |
| pcbench-Dekada_dekada_TopoR_curves | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.3 | +0.0 | 0 | 379.6 | loss (cpu_s) |
| pcbench-DerKnopf_digi-pot | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 21.4 | | 18 | 467.6 | baseline |
| pcbench-DerKnopf_digi-pot | kicad | neckdown | 0.00 | 0 | 1 | 995.9 | -4.1 | | 19.8 | -1.6 | 13 | 455.6 | loss (clean_pass_rate) |
| pcbench-DerKnopf_led-ring | kicad | main | 1.00 | 0 | 0 | 999.9 | | unmeasured | 118.2 | | 38 | 550.7 | baseline |
| pcbench-DerKnopf_led-ring | kicad | neckdown | 1.00 | 0 | 0 | 999.9 | 0.0 | | 120.1 | +1.8 | 38 | 550.7 | loss (cpu_s) |
| pcbench-DerKnopf_power-supply | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.0 | | 0 | 189.9 | baseline |
| pcbench-DerKnopf_power-supply | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.1 | +0.1 | 0 | 189.9 | loss (cpu_s) |
| pcbench-DiscoDanceFloorV1_DiscoDongle | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 10.3 | | 9 | 463.5 | baseline |
| pcbench-DiscoDanceFloorV1_DiscoDongle | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 10.4 | +0.2 | 9 | 463.5 | loss (cpu_s) |
| pcbench-DonCon2040_DonConPad | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 5.6 | | 6 | 574.5 | baseline |
| pcbench-DonCon2040_DonConPad | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 5.7 | +0.1 | 6 | 574.5 | loss (cpu_s) |
| pcbench-Doorman_doorman_slot | kicad | main | 0.00 | 6 | 0 | 875.0 | | unmeasured | 9.9 | | 18 | 196.7 | baseline |
| pcbench-Doorman_doorman_slot | kicad | neckdown | 0.00 | 6 | 0 | 875.0 | 0.0 | | 10.2 | +0.2 | 18 | 196.7 | loss (cpu_s) |
| pcbench-DoroidOscillo-Board_Android_Oscilloscope | kicad | main | 0.00 | 5 | 0 | 935.0 | | unmeasured | 212.1 | | 87 | 1774.4 | baseline |
| pcbench-DoroidOscillo-Board_Android_Oscilloscope | kicad | neckdown | 0.00 | 5 | 0 | 935.0 | 0.0 | | 213.3 | +1.2 | 87 | 1774.4 | loss (cpu_s) |
| pcbench-DualLM317BenchSupply_DualLM317BenchSupply | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 4.1 | | 0 | 1089.3 | baseline |
| pcbench-DualLM317BenchSupply_DualLM317BenchSupply | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 4.3 | +0.2 | 0 | 1089.3 | loss (cpu_s) |
| pcbench-DustSensorShield_DustSensorShield | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.9 | | 3 | 491.8 | baseline |
| pcbench-DustSensorShield_DustSensorShield | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.7 | -0.1 | 3 | 491.8 | win (cpu_s) |
| pcbench-E202VAR-Natural-Radio-Receiver_e202var-vlf-radio-receiver | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 7.1 | | 0 | 968.8 | baseline |
| pcbench-E202VAR-Natural-Radio-Receiver_e202var-vlf-radio-receiver | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 7.5 | +0.4 | 0 | 968.8 | loss (cpu_s) |
| pcbench-EEGFrontier_EEGFrontier | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 24.8 | | 31 | 628.7 | baseline |
| pcbench-EEGFrontier_EEGFrontier | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 25.4 | +0.6 | 31 | 628.7 | loss (cpu_s) |
| pcbench-ESP-12-breakout_ESP12E-breakout | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.2 | | 2 | 347.7 | baseline |
| pcbench-ESP-12-breakout_ESP12E-breakout | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.3 | +0.1 | 2 | 347.7 | loss (cpu_s) |
| pcbench-ESP-Breakout_ESP-Breakout | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 3.7 | | 16 | 346.6 | baseline |
| pcbench-ESP-Breakout_ESP-Breakout | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 3.7 | +0.1 | 16 | 346.6 | loss (cpu_s) |
| pcbench-ESP07-Breakout_ESP07-Breakout | kicad | main | 0.00 | 0 | 2 | 982.6 | | unmeasured | 4.7 | | 5 | 741.1 | baseline |
| pcbench-ESP07-Breakout_ESP07-Breakout | kicad | neckdown | 0.00 | 0 | 2 | 982.6 | 0.0 | | 4.9 | +0.2 | 5 | 741.1 | loss (cpu_s) |
| pcbench-ESP32-Module-Breakout_ESP32S-breakout | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 11.9 | | 25 | 737.7 | baseline |
| pcbench-ESP32-Module-Breakout_ESP32S-breakout | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 11.8 | -0.0 | 25 | 737.7 | win (cpu_s) |
| pcbench-ESP8266-MQTT-battery-monitor-hw_battery-monitor | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 10.1 | | 15 | 684.6 | baseline |
| pcbench-ESP8266-MQTT-battery-monitor-hw_battery-monitor | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 10.3 | +0.2 | 15 | 684.6 | loss (cpu_s) |
| pcbench-ESPLux_Board | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 7.9 | | 12 | 795.9 | baseline |
| pcbench-ESPLux_Board | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 8.2 | +0.3 | 12 | 795.9 | loss (cpu_s) |
| pcbench-ESP_BaPoTeSta_ESP_BaPoTeSta | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 31.1 | | 12 | 849.9 | baseline |
| pcbench-ESP_BaPoTeSta_ESP_BaPoTeSta | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 31.4 | +0.3 | 12 | 849.9 | loss (cpu_s) |
| pcbench-ESP_WiFiSwitch_WifiSwitch | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.1 | | 3 | 340.3 | baseline |
| pcbench-ESP_WiFiSwitch_WifiSwitch | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.2 | +0.0 | 3 | 340.3 | loss (cpu_s) |
| pcbench-Eggbot-Spherebot-polargraph-Controller_eggbot-spherebot-polargraph-controller | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 6.0 | | 0 | 1273.4 | baseline |
| pcbench-Eggbot-Spherebot-polargraph-Controller_eggbot-spherebot-polargraph-controller | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 6.2 | +0.2 | 0 | 1273.4 | loss (cpu_s) |
| pcbench-Electronics-MainBoard_MainBoard | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 94.9 | | 31 | 3076.9 | baseline |
| pcbench-Electronics-MainBoard_MainBoard | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 108.5 | +13.6 | 35 | 3016.2 | loss (score) |
| pcbench-EncoderBoard_Enc_Pan_Led | kicad | main | 0.00 | 1 | 0 | 977.3 | | unmeasured | 131.8 | | 36 | 1046.3 | baseline |
| pcbench-EncoderBoard_Enc_Pan_Led | kicad | neckdown | 0.00 | 1 | 0 | 977.3 | 0.0 | | 131.6 | -0.2 | 36 | 1046.3 | win (cpu_s) |
| pcbench-EnvOpenPico_EliteMicro2040 | kicad | main | 0.00 | 18 | 0 | 684.2 | | unmeasured | 68.7 | | 24 | 426.5 | baseline |
| pcbench-EnvOpenPico_EliteMicro2040 | kicad | neckdown | 0.00 | 18 | 0 | 684.2 | 0.0 | | 69.2 | +0.5 | 24 | 426.5 | loss (cpu_s) |
| pcbench-EuroPi_europi-surface-mount | kicad | main | 0.00 | 27 | 18 | 656.2 | | unmeasured | 206.4 | | 55 | 2623.9 | baseline |
| pcbench-EuroPi_europi-surface-mount | kicad | neckdown | 0.00 | 27 | 18 | 656.2 | 0.0 | | 199.2 | -7.2 | 55 | 2623.9 | win (cpu_s) |
| pcbench-FRM16_Relay_Module_I2C_Controller_relay_controller | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 23.0 | | 5 | 1442.8 | baseline |
| pcbench-FRM16_Relay_Module_I2C_Controller_relay_controller | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 23.5 | +0.5 | 5 | 1442.8 | loss (cpu_s) |
| pcbench-FT231X_breakout_FTDI_FT231XS-U_Breakout | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 10.5 | | 10 | 413.7 | baseline |
| pcbench-FT231X_breakout_FTDI_FT231XS-U_Breakout | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 10.5 | -0.0 | 10 | 413.7 | win (cpu_s) |
| pcbench-Feather-ICE40-PCB_feather_ice40 | kicad | main | 0.00 | 26 | 0 | 657.9 | | unmeasured | 100.3 | | 37 | 869.2 | baseline |
| pcbench-Feather-ICE40-PCB_feather_ice40 | kicad | neckdown | 0.00 | 26 | 0 | 657.9 | 0.0 | | 100.9 | +0.6 | 37 | 869.2 | loss (cpu_s) |
| pcbench-FlashProgrammer_flash_programmer | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 54.6 | | 42 | 1598.9 | baseline |
| pcbench-FlashProgrammer_flash_programmer | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 54.8 | +0.3 | 42 | 1598.9 | loss (cpu_s) |
| pcbench-FogDrive_attiny45_slim | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.0 | | 0 | 52.5 | baseline |
| pcbench-FogDrive_attiny45_slim | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.1 | +0.0 | 0 | 52.5 | loss (cpu_s) |
| pcbench-GameTiger_GameTiger | kicad | main | 0.00 | 26 | 199 | 110.8 | | unmeasured | 97.1 | | 43 | 1571.3 | baseline |
| pcbench-GameTiger_GameTiger | kicad | neckdown | 0.00 | 26 | 199 | 110.8 | 0.0 | | 97.9 | +0.8 | 43 | 1571.3 | loss (cpu_s) |
| pcbench-HES-V2__autosave-hes | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.4 | | 0 | 456.2 | baseline |
| pcbench-HES-V2__autosave-hes | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.4 | +0.0 | 0 | 456.2 | loss (cpu_s) |
| pcbench-HES-V2_hes | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.4 | | 0 | 456.2 | baseline |
| pcbench-HES-V2_hes | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.4 | +0.0 | 0 | 456.2 | loss (cpu_s) |
| pcbench-HW-AC-Emeter_ac-power-monitor | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 30.5 | | 15 | 1222.3 | baseline |
| pcbench-HW-AC-Emeter_ac-power-monitor | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 31.1 | +0.6 | 15 | 1222.3 | loss (cpu_s) |
| pcbench-Hangul-Clock_Hangul | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 60.2 | | 38 | 3484.2 | baseline |
| pcbench-Hangul-Clock_Hangul | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 60.9 | +0.7 | 38 | 3484.2 | loss (cpu_s) |
| pcbench-Hardware-done-with-kicad_AVRlearn | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.2 | | 0 | 398.2 | baseline |
| pcbench-Hardware-done-with-kicad_AVRlearn | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.3 | +0.1 | 0 | 398.2 | loss (cpu_s) |
| pcbench-Hardware_Playground_BL_PCB_latest | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 17.3 | | 10 | 1891.4 | baseline |
| pcbench-Hardware_Playground_BL_PCB_latest | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 17.1 | -0.3 | 10 | 1891.4 | win (cpu_s) |
| pcbench-Hardware_Playground_Touch_Switch_1ch_PCB | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 38.8 | | 22 | 672.2 | baseline |
| pcbench-Hardware_Playground_Touch_Switch_1ch_PCB | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 39.8 | +0.9 | 22 | 672.2 | loss (cpu_s) |
| pcbench-Hardware_Playground_buck_led_driver | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.4 | | 3 | 100.6 | baseline |
| pcbench-Hardware_Playground_buck_led_driver | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.4 | +0.0 | 3 | 100.6 | loss (cpu_s) |
| pcbench-Hardware_Playground_esp8266_uno_relay | kicad | main | 0.00 | 1 | 0 | 944.4 | | unmeasured | 41.1 | | 0 | 591.0 | baseline |
| pcbench-Hardware_Playground_esp8266_uno_relay | kicad | neckdown | 0.00 | 1 | 0 | 944.4 | 0.0 | | 41.6 | +0.4 | 0 | 591.0 | loss (cpu_s) |
| pcbench-Hardware_Playground_hy_adapter | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.5 | | 7 | 65.9 | baseline |
| pcbench-Hardware_Playground_hy_adapter | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.5 | -0.0 | 7 | 65.9 | win (cpu_s) |
| pcbench-Hardware_Playground_minimal_node_rfm69w | kicad | main | 0.00 | 3 | 0 | 900.0 | | unmeasured | 77.0 | | 24 | 570.7 | baseline |
| pcbench-Hardware_Playground_minimal_node_rfm69w | kicad | neckdown | 0.00 | 3 | 0 | 900.0 | 0.0 | | 77.3 | +0.2 | 24 | 570.7 | loss (cpu_s) |
| pcbench-Hardware_Playground_nrf52832_uno | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 31.6 | | 27 | 1637.4 | baseline |
| pcbench-Hardware_Playground_nrf52832_uno | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 32.1 | +0.5 | 27 | 1637.4 | loss (cpu_s) |
| pcbench-Hardware_Playground_orange_pi_zero_node | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 6.2 | | 1 | 826.5 | baseline |
| pcbench-Hardware_Playground_orange_pi_zero_node | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 6.4 | +0.1 | 1 | 826.5 | loss (cpu_s) |
| pcbench-Hardware_Playground_pro_mini | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 31.2 | | 2 | 465.9 | baseline |
| pcbench-Hardware_Playground_pro_mini | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 32.6 | +1.4 | 2 | 465.9 | loss (cpu_s) |
| pcbench-Hardware_Playground_rpi_zero | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 49.6 | | 3 | 876.4 | baseline |
| pcbench-Hardware_Playground_rpi_zero | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 50.7 | +1.1 | 3 | 876.4 | loss (cpu_s) |
| pcbench-Hardware_Playground_serial_gw_ATMEGA328P | kicad | main | 0.00 | 1 | 0 | 969.7 | | unmeasured | 110.6 | | 35 | 738.0 | baseline |
| pcbench-Hardware_Playground_serial_gw_ATMEGA328P | kicad | neckdown | 0.00 | 1 | 2 | 957.6 | -12.1 | | 118.3 | +7.7 | 32 | 758.2 | loss (violations) |
| pcbench-Hardware_Playground_serial_gw_maple_mini | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 25.7 | | 1 | 901.9 | baseline |
| pcbench-Hardware_Playground_serial_gw_maple_mini | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 25.9 | +0.1 | 1 | 901.9 | loss (cpu_s) |
| pcbench-Hardware_Playground_usb_shield | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 8.8 | | 0 | 707.2 | baseline |
| pcbench-Hardware_Playground_usb_shield | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 9.1 | +0.3 | 0 | 707.2 | loss (cpu_s) |
| pcbench-Hardware_Playground_wifi_lights | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.6 | | 6 | 633.6 | baseline |
| pcbench-Hardware_Playground_wifi_lights | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.7 | +0.0 | 6 | 633.6 | loss (cpu_s) |
| pcbench-HaveSome_PCB_HaveSomePCB | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.7 | | 0 | 86.0 | baseline |
| pcbench-HaveSome_PCB_HaveSomePCB | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.7 | -0.0 | 0 | 86.0 | win (cpu_s) |
| pcbench-HellScribe_HellScribe | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 14.1 | | 24 | 1548.0 | baseline |
| pcbench-HellScribe_HellScribe | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 14.2 | +0.1 | 24 | 1548.0 | loss (cpu_s) |
| pcbench-HillhacksLantern_LEDLantern | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.9 | | 0 | 426.5 | baseline |
| pcbench-HillhacksLantern_LEDLantern | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.9 | +0.0 | 0 | 426.5 | loss (cpu_s) |
| pcbench-Hubble_12_bit_analog_out | kicad | main | 0.00 | 5 | 0 | 807.7 | | unmeasured | 24.3 | | 14 | 387.3 | baseline |
| pcbench-Hubble_12_bit_analog_out | kicad | neckdown | 0.00 | 5 | 0 | 807.7 | 0.0 | | 24.4 | +0.0 | 14 | 387.3 | loss (cpu_s) |
| pcbench-Hubble_16_bit_analog_out | kicad | main | 0.00 | 7 | 0 | 740.7 | | unmeasured | 21.1 | | 12 | 387.5 | baseline |
| pcbench-Hubble_16_bit_analog_out | kicad | neckdown | 0.00 | 7 | 0 | 740.7 | 0.0 | | 21.6 | +0.6 | 12 | 387.5 | loss (cpu_s) |
| pcbench-Hubble_jacks | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.2 | | 0 | 72.3 | baseline |
| pcbench-Hubble_jacks | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.2 | 0.0 | 0 | 72.3 | tie (peak_rss_mb) |
| pcbench-Hubble_leds | kicad | main | 0.00 | 4 | 0 | 851.8 | | unmeasured | 18.0 | | 9 | 535.2 | baseline |
| pcbench-Hubble_leds | kicad | neckdown | 0.00 | 4 | 0 | 851.8 | 0.0 | | 18.2 | +0.1 | 9 | 535.2 | loss (cpu_s) |
| pcbench-Hubble_mux | kicad | main | 0.00 | 1 | 0 | 941.2 | | unmeasured | 8.8 | | 6 | 255.5 | baseline |
| pcbench-Hubble_mux | kicad | neckdown | 0.00 | 1 | 0 | 941.2 | 0.0 | | 8.8 | +0.1 | 6 | 255.5 | loss (cpu_s) |
| pcbench-Hypfer-RGB-W-LED-Controller_led_strip_controller | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 3.8 | | 2 | 1146.6 | baseline |
| pcbench-Hypfer-RGB-W-LED-Controller_led_strip_controller | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 4.1 | +0.3 | 2 | 1146.6 | loss (cpu_s) |
| pcbench-I2CTempsensor_sensors | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.6 | | 0 | 140.3 | baseline |
| pcbench-I2CTempsensor_sensors | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.6 | -0.0 | 0 | 140.3 | win (cpu_s) |
| pcbench-ID-FIX_scanConnect | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.0 | | 0 | 50.8 | baseline |
| pcbench-ID-FIX_scanConnect | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.1 | +0.0 | 0 | 50.8 | loss (cpu_s) |
| pcbench-IR-Transponder-ATTiny85-v2_Transponder_v2 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.8 | | 1 | 135.3 | baseline |
| pcbench-IR-Transponder-ATTiny85-v2_Transponder_v2 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.8 | 0.0 | 1 | 135.3 | win (peak_rss_mb) |
| pcbench-ISO-port_ch340-usb-serial-isolated | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 6.6 | | 7 | 682.5 | baseline |
| pcbench-ISO-port_ch340-usb-serial-isolated | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 6.8 | +0.2 | 7 | 682.5 | loss (cpu_s) |
| pcbench-Inhibition_amplifier | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 11.4 | | 1 | 2400.6 | baseline |
| pcbench-Inhibition_amplifier | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 11.5 | +0.1 | 1 | 2400.6 | loss (cpu_s) |
| pcbench-Inkjet_InkjetBreakout | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.4 | | 1 | 255.4 | baseline |
| pcbench-Inkjet_InkjetBreakout | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.4 | +0.0 | 1 | 255.4 | loss (cpu_s) |
| pcbench-Inkjet_InkjetDriver | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 14.9 | | 21 | 1162.3 | baseline |
| pcbench-Inkjet_InkjetDriver | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 15.2 | +0.3 | 21 | 1162.3 | loss (cpu_s) |
| pcbench-Inkjet_PiezoDriver | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 3.7 | | 4 | 288.1 | baseline |
| pcbench-Inkjet_PiezoDriver | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 3.7 | +0.1 | 4 | 288.1 | loss (cpu_s) |
| pcbench-Inkjet__autosave-InkjetDriver | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 14.8 | | 21 | 1162.3 | baseline |
| pcbench-Inkjet__autosave-InkjetDriver | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 15.3 | +0.5 | 21 | 1162.3 | loss (cpu_s) |
| pcbench-JLink-SWD_JLink-SWD | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.8 | | 4 | 300.6 | baseline |
| pcbench-JLink-SWD_JLink-SWD | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.8 | -0.0 | 4 | 300.6 | win (cpu_s) |
| pcbench-Kefersender_UKW TX | kicad | main | 0.00 | 5 | 0 | 814.8 | | unmeasured | 42.3 | | 0 | 407.0 | baseline |
| pcbench-Kefersender_UKW TX | kicad | neckdown | 0.00 | 5 | 0 | 814.8 | 0.0 | | 42.9 | +0.6 | 0 | 407.0 | loss (cpu_s) |
| pcbench-Keyboard_PCB_Keyboard | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 170.4 | | 55 | 12076.8 | baseline |
| pcbench-Keyboard_PCB_Keyboard | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 170.4 | +0.0 | 55 | 12076.8 | loss (cpu_s) |
| pcbench-KiCad-LTC6802-2_main | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.7 | | 3 | 450.6 | baseline |
| pcbench-KiCad-LTC6802-2_main | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.6 | -0.1 | 3 | 450.6 | win (cpu_s) |
| pcbench-KosselHotendBoard_KosselHotendPCB | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.2 | | 0 | 208.6 | baseline |
| pcbench-KosselHotendBoard_KosselHotendPCB | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.2 | 0.0 | 0 | 208.6 | win (peak_rss_mb) |
| pcbench-L6235-PCB_L6235 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.5 | | 0 | 699.3 | baseline |
| pcbench-L6235-PCB_L6235 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.6 | +0.1 | 0 | 699.3 | loss (cpu_s) |
| pcbench-LAUNCHXL-F28027-isolation-PCB_project1 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.3 | | 0 | 184.2 | baseline |
| pcbench-LAUNCHXL-F28027-isolation-PCB_project1 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.3 | 0.0 | 0 | 184.2 | loss (peak_rss_mb) |
| pcbench-LED-Square_PT4115_LED-Square_PT4115 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 3.8 | | 0 | 433.9 | baseline |
| pcbench-LED-Square_PT4115_LED-Square_PT4115 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 3.8 | 0.0 | 0 | 433.9 | loss (peak_rss_mb) |
| pcbench-LPC2148_Stick_LPC2148_stick | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 249.0 | | 91 | 4652.0 | baseline |
| pcbench-LPC2148_Stick_LPC2148_stick | kicad | neckdown | 0.00 | 0 | 3 | 993.8 | -6.2 | | 219.8 | -29.2 | 109 | 4846.2 | loss (clean_pass_rate) |
| pcbench-LPC2148_Stick__autosave-LPC2148_stick | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 249.4 | | 91 | 4652.0 | baseline |
| pcbench-LPC2148_Stick__autosave-LPC2148_stick | kicad | neckdown | 0.00 | 0 | 3 | 993.8 | -6.2 | | 222.8 | -26.6 | 109 | 4846.2 | loss (clean_pass_rate) |
| pcbench-LT3652EvalBoard_LT3652EvalBoard | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 10.1 | | 4 | 617.3 | baseline |
| pcbench-LT3652EvalBoard_LT3652EvalBoard | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 10.5 | +0.4 | 4 | 617.3 | loss (cpu_s) |
| pcbench-LVDS2TMDS_LVDS2TMDS | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 3.5 | | 12 | 405.1 | baseline |
| pcbench-LVDS2TMDS_LVDS2TMDS | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 3.6 | +0.1 | 12 | 405.1 | loss (cpu_s) |
| pcbench-LadyBugShield_LBS-TEST1 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 5.4 | | 4 | 439.2 | baseline |
| pcbench-LadyBugShield_LBS-TEST1 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 5.4 | 0.0 | 4 | 439.2 | loss (peak_rss_mb) |
| pcbench-LadybugLiteBlue_HW_LadybugBlueLite | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 36.1 | | 17 | 1097.6 | baseline |
| pcbench-LadybugLiteBlue_HW_LadybugBlueLite | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 36.3 | +0.2 | 17 | 1097.6 | loss (cpu_s) |
| pcbench-LiFePO4-Charge-Controller_LiFePO4-Charge-Controller | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 4.9 | | 3 | 798.7 | baseline |
| pcbench-LiFePO4-Charge-Controller_LiFePO4-Charge-Controller | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 4.9 | +0.0 | 3 | 798.7 | loss (cpu_s) |
| pcbench-Librecalc-Hardware__autosave-calculator | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 298.5 | | 236 | 4407.5 | baseline |
| pcbench-Librecalc-Hardware__autosave-calculator | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 287.5 | -11.0 | 236 | 4406.5 | win (score) |
| pcbench-LimitSwitchesPlugin_LimitSwitchesPlugin | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.7 | | 1 | 719.5 | baseline |
| pcbench-LimitSwitchesPlugin_LimitSwitchesPlugin | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.7 | +0.0 | 1 | 719.5 | loss (cpu_s) |
| pcbench-LittleArduinoProjects_LEDx16_board | kicad | main | 0.00 | 7 | 0 | 820.5 | | unmeasured | 3.5 | | 8 | 378.2 | baseline |
| pcbench-LittleArduinoProjects_LEDx16_board | kicad | neckdown | 0.00 | 7 | 0 | 820.5 | 0.0 | | 3.5 | 0.0 | 8 | 378.2 | tie (peak_rss_mb) |
| pcbench-LittleArduinoProjects_sevensegment_led_display_module | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.3 | | 0 | 313.6 | baseline |
| pcbench-LittleArduinoProjects_sevensegment_led_display_module | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.3 | +0.0 | 0 | 313.6 | loss (cpu_s) |
| pcbench-LoRaCatTrack_GPSLoRa | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 9.8 | | 9 | 1303.7 | baseline |
| pcbench-LoRaCatTrack_GPSLoRa | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 9.9 | +0.1 | 9 | 1303.7 | loss (cpu_s) |
| pcbench-LoRaPP_loramod | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 74.4 | | 21 | 744.7 | baseline |
| pcbench-LoRaPP_loramod | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 74.9 | +0.5 | 21 | 744.7 | loss (cpu_s) |
| pcbench-LongPixel_AnalogDriverMini | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 4.2 | | 1 | 409.8 | baseline |
| pcbench-LongPixel_AnalogDriverMini | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 4.3 | +0.1 | 1 | 409.8 | loss (cpu_s) |
| pcbench-MAGFest-2017-Swadges_magfest_badges | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 17.1 | | 16 | 1280.2 | baseline |
| pcbench-MAGFest-2017-Swadges_magfest_badges | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 17.6 | +0.5 | 16 | 1280.2 | loss (cpu_s) |
| pcbench-MAVRIC_Hardware_ArduinoPracticeBoard | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 13.7 | | 13 | 743.5 | baseline |
| pcbench-MAVRIC_Hardware_ArduinoPracticeBoard | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 13.8 | +0.2 | 13 | 743.5 | loss (cpu_s) |
| pcbench-MAVRIC_Hardware_Motherboard | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 56.4 | | 18 | 2115.6 | baseline |
| pcbench-MAVRIC_Hardware_Motherboard | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 57.4 | +1.0 | 18 | 2115.6 | loss (cpu_s) |
| pcbench-MAVRIC_Hardware_SoilBoard | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.8 | | 7 | 371.3 | baseline |
| pcbench-MAVRIC_Hardware_SoilBoard | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.9 | +0.1 | 7 | 371.3 | loss (cpu_s) |
| pcbench-MAVRIC_Hardware__autosave-Motherboard | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 56.8 | | 18 | 2115.6 | baseline |
| pcbench-MAVRIC_Hardware__autosave-Motherboard | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 57.4 | +0.6 | 18 | 2115.6 | loss (cpu_s) |
| pcbench-MSGEQ7-Breakout-Board_MSGEQ7_Breakout_Board | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.1 | | 0 | 190.3 | baseline |
| pcbench-MSGEQ7-Breakout-Board_MSGEQ7_Breakout_Board | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.2 | +0.0 | 0 | 190.3 | loss (cpu_s) |
| pcbench-Mechaduino-DR_Mechaduino DR 1.01 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 79.8 | | 58 | 1433.9 | baseline |
| pcbench-Mechaduino-DR_Mechaduino DR 1.01 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 80.9 | +1.1 | 58 | 1433.9 | loss (cpu_s) |
| pcbench-Minitel_bbb-adapter | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 3.7 | | 2 | 964.5 | baseline |
| pcbench-Minitel_bbb-adapter | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 3.7 | -0.0 | 2 | 964.5 | win (cpu_s) |
| pcbench-Minitel_driver_board | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.5 | | 1 | 847.0 | baseline |
| pcbench-Minitel_driver_board | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.5 | +0.0 | 1 | 847.0 | loss (cpu_s) |
| pcbench-MixSID_mixsid | kicad | main | 0.00 | 8 | 0 | 924.5 | | unmeasured | 171.1 | | 40 | 2662.3 | baseline |
| pcbench-MixSID_mixsid | kicad | neckdown | 0.00 | 8 | 0 | 924.5 | 0.0 | | 169.8 | -1.3 | 40 | 2662.3 | win (cpu_s) |
| pcbench-Mouse_Mouse | kicad | main | 0.00 | 4 | 0 | 970.8 | | unmeasured | 50.0 | | 29 | 924.6 | baseline |
| pcbench-Mouse_Mouse | kicad | neckdown | 0.00 | 4 | 0 | 970.8 | 0.0 | | 50.3 | +0.2 | 29 | 924.6 | loss (cpu_s) |
| pcbench-MySRaspiGW_MySRaspiGW | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 3.9 | | 4 | 87.5 | baseline |
| pcbench-MySRaspiGW_MySRaspiGW | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 4.0 | +0.1 | 4 | 87.5 | loss (cpu_s) |
| pcbench-MySRaspiGW_MySRaspiGW_PA_LNA | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.7 | | 2 | 90.9 | baseline |
| pcbench-MySRaspiGW_MySRaspiGW_PA_LNA | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.7 | +0.0 | 2 | 90.9 | loss (cpu_s) |
| pcbench-MySRaspiGW_MySRaspiGW_PA_LNA_Pimoroni | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.1 | | 2 | 108.7 | baseline |
| pcbench-MySRaspiGW_MySRaspiGW_PA_LNA_Pimoroni | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.1 | -0.0 | 2 | 108.7 | win (cpu_s) |
| pcbench-MySRaspiGW_MySRaspiGW_Pimoroni | kicad | main | 0.00 | 1 | 0 | 888.9 | | unmeasured | 4.1 | | 3 | 96.5 | baseline |
| pcbench-MySRaspiGW_MySRaspiGW_Pimoroni | kicad | neckdown | 0.00 | 1 | 0 | 888.9 | 0.0 | | 4.2 | +0.1 | 3 | 96.5 | loss (cpu_s) |
| pcbench-NRC2016_banked_ram | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 73.9 | | 25 | 3947.5 | baseline |
| pcbench-NRC2016_banked_ram | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 75.1 | +1.2 | 25 | 3947.5 | loss (cpu_s) |
| pcbench-NRC2016_usb_sio | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 19.5 | | 11 | 1903.6 | baseline |
| pcbench-NRC2016_usb_sio | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 19.7 | +0.2 | 11 | 1903.6 | loss (cpu_s) |
| pcbench-NRC2016_z80 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 117.1 | | 30 | 5693.7 | baseline |
| pcbench-NRC2016_z80 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 116.6 | -0.5 | 30 | 5693.7 | win (cpu_s) |
| pcbench-NavigationThing_NavigationThing | kicad | main | 0.00 | 0 | 3 | 977.8 | | unmeasured | 20.2 | | 1 | 854.1 | baseline |
| pcbench-NavigationThing_NavigationThing | kicad | neckdown | 0.00 | 0 | 3 | 977.8 | 0.0 | | 20.6 | +0.3 | 1 | 854.1 | loss (cpu_s) |
| pcbench-NeoWall_NeoWall | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.8 | | 0 | 598.0 | baseline |
| pcbench-NeoWall_NeoWall | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.8 | +0.0 | 0 | 598.0 | loss (cpu_s) |
| pcbench-Neptune-Hardware_DataAcquisitionBoard | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.4 | | 0 | 683.9 | baseline |
| pcbench-Neptune-Hardware_DataAcquisitionBoard | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.4 | -0.0 | 0 | 683.9 | win (cpu_s) |
| pcbench-NiMH-Charger_NiMH Charger | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.5 | | 0 | 854.7 | baseline |
| pcbench-NiMH-Charger_NiMH Charger | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.5 | +0.0 | 0 | 854.7 | loss (cpu_s) |
| pcbench-OLD-Stepper-motor-board-design-project_Stepper motor driver | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 9.2 | | 17 | 544.8 | baseline |
| pcbench-OLD-Stepper-motor-board-design-project_Stepper motor driver | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 9.4 | +0.2 | 17 | 544.8 | loss (cpu_s) |
| pcbench-OSHW-reCamera-Series_reCamera_Basically_Board | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 7.8 | | 8 | 254.3 | baseline |
| pcbench-OSHW-reCamera-Series_reCamera_Basically_Board | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 7.8 | -0.0 | 8 | 254.3 | win (cpu_s) |
| pcbench-Omega2-Berrydock_berrydock-mini | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 150.0 | | 48 | 1547.8 | baseline |
| pcbench-Omega2-Berrydock_berrydock-mini | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 153.5 | +3.5 | 48 | 1547.8 | loss (cpu_s) |
| pcbench-Omega2-mini-dock_Omega2 mini-dock | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 9.4 | | 0 | 579.7 | baseline |
| pcbench-Omega2-mini-dock_Omega2 mini-dock | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 9.1 | -0.3 | 0 | 579.7 | win (cpu_s) |
| pcbench-OpAmpPassXsistorBenchSupply_OpAmpPassXsistorBenchSupply | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 11.6 | | 1 | 1591.2 | baseline |
| pcbench-OpAmpPassXsistorBenchSupply_OpAmpPassXsistorBenchSupply | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 11.8 | +0.1 | 1 | 1591.2 | loss (cpu_s) |
| pcbench-Open-Source-Power-Supply_PowerSupply_PCB | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 4.7 | | 2 | 1066.9 | baseline |
| pcbench-Open-Source-Power-Supply_PowerSupply_PCB | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 4.6 | -0.0 | 2 | 1066.9 | win (cpu_s) |
| pcbench-Open-Source-Power-Supply_PowerSupply_PCB_backup | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 4.5 | | 3 | 1072.0 | baseline |
| pcbench-Open-Source-Power-Supply_PowerSupply_PCB_backup | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 4.7 | +0.2 | 3 | 1072.0 | loss (cpu_s) |
| pcbench-OpenHardwareExG_ActiveElectrode_OpenHardwareExG_ActiveElectrode | kicad | main | 0.00 | 2 | 0 | 800.0 | | unmeasured | 30.8 | | 0 | 134.9 | baseline |
| pcbench-OpenHardwareExG_ActiveElectrode_OpenHardwareExG_ActiveElectrode | kicad | neckdown | 0.00 | 2 | 0 | 800.0 | 0.0 | | 31.2 | +0.4 | 0 | 134.9 | loss (cpu_s) |
| pcbench-OpenHardwareExG_Shield_OpenHardwareExG_Shield | kicad | main | 0.00 | 3 | 34 | 930.0 | | unmeasured | 192.9 | | 95 | 3129.4 | baseline |
| pcbench-OpenHardwareExG_Shield_OpenHardwareExG_Shield | kicad | neckdown | 0.00 | 1 | 30 | 950.0 | +20.0 | | 179.1 | -13.8 | 102 | 3121.3 | win (unrouted) |
| pcbench-OpenHardwareExG_Shield_OpenHardwareExG_Shield_Test_Board | kicad | main | 0.00 | 0 | 2 | 996.2 | | unmeasured | 49.9 | | 64 | 3682.0 | baseline |
| pcbench-OpenHardwareExG_Shield_OpenHardwareExG_Shield_Test_Board | kicad | neckdown | 0.00 | 0 | 2 | 996.2 | 0.0 | | 50.5 | +0.6 | 64 | 3682.0 | loss (cpu_s) |
| pcbench-OpenHardwareExG_Shield_OpenHardwareExG_Shield_Test_Board_all_panelled | kicad | main | 0.00 | 0 | 2 | 996.2 | | unmeasured | 50.1 | | 64 | 3682.0 | baseline |
| pcbench-OpenHardwareExG_Shield_OpenHardwareExG_Shield_Test_Board_all_panelled | kicad | neckdown | 0.00 | 0 | 2 | 996.2 | 0.0 | | 50.6 | +0.5 | 64 | 3682.0 | loss (cpu_s) |
| pcbench-OpenVNAVI_driver unit | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 130.8 | | 35 | 2926.3 | baseline |
| pcbench-OpenVNAVI_driver unit | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 130.8 | -0.0 | 35 | 2926.3 | win (cpu_s) |
| pcbench-OpenVNAVI_motor unit | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.2 | | 0 | 59.0 | baseline |
| pcbench-OpenVNAVI_motor unit | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.2 | 0.0 | 0 | 59.0 | tie (peak_rss_mb) |
| pcbench-Own-Mailbox-Hardware_eth | kicad | main | 0.00 | 1 | 0 | 993.6 | | unmeasured | 282.3 | | 184 | 2321.0 | baseline |
| pcbench-Own-Mailbox-Hardware_eth | kicad | neckdown | 0.00 | 1 | 0 | 993.6 | 0.0 | | 240.9 | -41.5 | 184 | 2321.0 | win (cpu_s) |
| pcbench-Own-Mailbox-Hardware_mailbox | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 260.6 | | 175 | 2363.2 | baseline |
| pcbench-Own-Mailbox-Hardware_mailbox | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 228.4 | -32.2 | 175 | 2363.2 | win (cpu_s) |
| pcbench-PCB_constant_current_ac_hv | kicad | main | 0.00 | 0 | 8 | 680.0 | | unmeasured | 0.1 | | 0 | 138.3 | baseline |
| pcbench-PCB_constant_current_ac_hv | kicad | neckdown | 0.00 | 0 | 8 | 680.0 | 0.0 | | 0.1 | +0.0 | 0 | 138.3 | loss (cpu_s) |
| pcbench-PCB_serie_led_strip | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.0 | | 0 | 39.1 | baseline |
| pcbench-PCB_serie_led_strip | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.0 | 0.0 | 0 | 39.1 | win (peak_rss_mb) |
| pcbench-PCB_small_halogen_replacement | kicad | main | 0.00 | 6 | 0 | 833.3 | | unmeasured | 70.3 | | 1 | 295.9 | baseline |
| pcbench-PCB_small_halogen_replacement | kicad | neckdown | 0.00 | 6 | 0 | 833.3 | 0.0 | | 70.6 | +0.2 | 1 | 295.9 | loss (cpu_s) |
| pcbench-PGA2311_pga2311 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 3.4 | | 6 | 539.0 | baseline |
| pcbench-PGA2311_pga2311 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 3.4 | +0.0 | 6 | 539.0 | loss (cpu_s) |
| pcbench-POV_POV | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.3 | | 0 | 334.0 | baseline |
| pcbench-POV_POV | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.3 | -0.0 | 0 | 334.0 | win (cpu_s) |
| pcbench-PWRmeter_PWMeter | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.8 | | 0 | 528.7 | baseline |
| pcbench-PWRmeter_PWMeter | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.8 | +0.0 | 0 | 528.7 | loss (cpu_s) |
| pcbench-Paperino_HW_Paperino_shield | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.2 | | 1 | 560.3 | baseline |
| pcbench-Paperino_HW_Paperino_shield | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.2 | +0.0 | 1 | 560.3 | loss (cpu_s) |
| pcbench-Paperino_HW_paperino_breakout | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.2 | | 0 | 163.4 | baseline |
| pcbench-Paperino_HW_paperino_breakout | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.2 | -0.0 | 0 | 163.4 | win (cpu_s) |
| pcbench-Patternflow_patternflow | kicad | main | 0.00 | 0 | 3 | 985.4 | | unmeasured | 4.0 | | 1 | 1190.4 | baseline |
| pcbench-Patternflow_patternflow | kicad | neckdown | 0.00 | 0 | 3 | 985.4 | 0.0 | | 4.1 | +0.1 | 1 | 1190.4 | loss (cpu_s) |
| pcbench-Phased-Array-Microphone-using-FPGA_SateliteMicrophone | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.1 | | 3 | 239.9 | baseline |
| pcbench-Phased-Array-Microphone-using-FPGA_SateliteMicrophone | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.2 | +0.1 | 3 | 239.9 | loss (cpu_s) |
| pcbench-Pi1541io_Pi1541io | kicad | main | 0.00 | 7 | 0 | 829.3 | | unmeasured | 113.7 | | 22 | 1523.9 | baseline |
| pcbench-Pi1541io_Pi1541io | kicad | neckdown | 0.00 | 7 | 0 | 829.3 | 0.0 | | 113.8 | +0.1 | 22 | 1523.9 | loss (cpu_s) |
| pcbench-Pi5_PCIe_Pi5_PCIe | kicad | main | 0.00 | 14 | 10 | 448.3 | | unmeasured | 29.7 | | 20 | 651.5 | baseline |
| pcbench-Pi5_PCIe_Pi5_PCIe | kicad | neckdown | 0.00 | 14 | 10 | 448.3 | 0.0 | | 30.0 | +0.2 | 20 | 651.5 | loss (cpu_s) |
| pcbench-PixyWirelessShield_Shield PIXY | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.1 | | 0 | 152.0 | baseline |
| pcbench-PixyWirelessShield_Shield PIXY | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.1 | +0.0 | 0 | 152.0 | loss (cpu_s) |
| pcbench-PmodHDMIIn_PmodHDMIIn | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 36.2 | | 18 | 887.9 | baseline |
| pcbench-PmodHDMIIn_PmodHDMIIn | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 36.6 | +0.4 | 18 | 887.9 | loss (cpu_s) |
| pcbench-PocketBone_pocketbone-kicad | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 216.8 | | 84 | 1610.6 | baseline |
| pcbench-PocketBone_pocketbone-kicad | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 197.4 | -19.3 | 84 | 1610.6 | win (cpu_s) |
| pcbench-Practicas-Curso-Kicad_Ejercicio_2 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 4.4 | | 7 | 520.2 | baseline |
| pcbench-Practicas-Curso-Kicad_Ejercicio_2 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 4.5 | +0.1 | 7 | 520.2 | loss (cpu_s) |
| pcbench-Practicas-Curso-Kicad_Salguero_Federico2 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 5.4 | | 6 | 592.8 | baseline |
| pcbench-Practicas-Curso-Kicad_Salguero_Federico2 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 5.6 | +0.2 | 6 | 592.8 | loss (cpu_s) |
| pcbench-Prototyping_Workshop_Prototyping_PCB | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 3.9 | | 0 | 680.2 | baseline |
| pcbench-Prototyping_Workshop_Prototyping_PCB | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 3.9 | +0.1 | 0 | 680.2 | loss (cpu_s) |
| pcbench-PsuFanController_FanController | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.2 | | 0 | 160.8 | baseline |
| pcbench-PsuFanController_FanController | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.2 | 0.0 | 0 | 160.8 | tie (peak_rss_mb) |
| pcbench-QRPCard_QRPCard | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 19.7 | | 4 | 1005.4 | baseline |
| pcbench-QRPCard_QRPCard | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 20.4 | +0.7 | 4 | 1005.4 | loss (cpu_s) |
| pcbench-R1002_R1002 | kicad | main | 0.00 | 3 | 0 | 880.0 | | unmeasured | 11.2 | | 18 | 532.0 | baseline |
| pcbench-R1002_R1002 | kicad | neckdown | 0.00 | 3 | 0 | 880.0 | 0.0 | | 11.1 | -0.0 | 18 | 532.0 | win (cpu_s) |
| pcbench-RC2014_RC2014 IDE | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 10.7 | | 4 | 1408.8 | baseline |
| pcbench-RC2014_RC2014 IDE | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 10.9 | +0.3 | 4 | 1408.8 | loss (cpu_s) |
| pcbench-RC2014_RC2014 RAM | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 5.6 | | 7 | 1971.6 | baseline |
| pcbench-RC2014_RC2014 RAM | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 5.7 | +0.1 | 7 | 1971.6 | loss (cpu_s) |
| pcbench-RC2014_RC2014 Tandy Sound Card | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 27.2 | | 14 | 2497.3 | baseline |
| pcbench-RC2014_RC2014 Tandy Sound Card | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 28.1 | +0.8 | 14 | 2497.3 | loss (cpu_s) |
| pcbench-RC6502-Apple-1-Replica_RC6502_Apple_1_SBC | kicad | main | 0.00 | 16 | 0 | 809.5 | | unmeasured | 298.3 | | 63 | 6209.7 | baseline |
| pcbench-RC6502-Apple-1-Replica_RC6502_Apple_1_SBC | kicad | neckdown | 0.00 | 16 | 0 | 809.5 | +0.0 | | 281.9 | -16.4 | 63 | 6204.9 | win (score) |
| pcbench-RC6502-Apple-1-Replica_RC6502_Terminal | kicad | main | 0.00 | 8 | 0 | 891.9 | | unmeasured | 18.8 | | 3 | 2590.0 | baseline |
| pcbench-RC6502-Apple-1-Replica_RC6502_Terminal | kicad | neckdown | 0.00 | 8 | 0 | 891.9 | 0.0 | | 19.0 | +0.2 | 3 | 2590.0 | loss (cpu_s) |
| pcbench-RC6502-Apple-1-Replica_RC6502_VDU | kicad | main | 0.00 | 18 | 0 | 793.1 | | unmeasured | 202.8 | | 54 | 5063.5 | baseline |
| pcbench-RC6502-Apple-1-Replica_RC6502_VDU | kicad | neckdown | 0.00 | 18 | 0 | 793.1 | 0.0 | | 190.4 | -12.4 | 54 | 5063.5 | win (cpu_s) |
| pcbench-RFM69HCW_ATSHA204A_Breakout_RFM69HCW_ATSHA204A_Breakout | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.1 | | 1 | 194.0 | baseline |
| pcbench-RFM69HCW_ATSHA204A_Breakout_RFM69HCW_ATSHA204A_Breakout | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.1 | -0.0 | 1 | 194.0 | win (cpu_s) |
| pcbench-RGBMatrixPanelCPLD-PhotonBackpack_RGBMatrixPanel_CPLD_negative | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 32.4 | | 16 | 1237.6 | baseline |
| pcbench-RGBMatrixPanelCPLD-PhotonBackpack_RGBMatrixPanel_CPLD_negative | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 33.0 | +0.6 | 16 | 1237.6 | loss (cpu_s) |
| pcbench-RGBMatrixPanelCPLD-PhotonBackpack_RGBMatrixPanel_CPLD_positive | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 29.5 | | 26 | 1211.4 | baseline |
| pcbench-RGBMatrixPanelCPLD-PhotonBackpack_RGBMatrixPanel_CPLD_positive | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 29.6 | +0.1 | 26 | 1211.4 | loss (cpu_s) |
| pcbench-RPi-PWM-Fan-interface_RPi PWM Fan interface | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.1 | | 0 | 74.8 | baseline |
| pcbench-RPi-PWM-Fan-interface_RPi PWM Fan interface | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.1 | +0.0 | 0 | 74.8 | loss (cpu_s) |
| pcbench-RX5808_diversityModule | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 13.3 | | 11 | 583.4 | baseline |
| pcbench-RX5808_diversityModule | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 12.8 | -0.5 | 12 | 574.0 | loss (score) |
| pcbench-RX5808_rx5808_4button | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 42.4 | | 44 | 1092.0 | baseline |
| pcbench-RX5808_rx5808_4button | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 43.1 | +0.7 | 44 | 1092.0 | loss (cpu_s) |
| pcbench-Raspberry-Pi-Soft-Power-Controller_Switching Supply TPS563208 MCI | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.4 | | 0 | 167.2 | baseline |
| pcbench-Raspberry-Pi-Soft-Power-Controller_Switching Supply TPS563208 MCI | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.4 | -0.0 | 0 | 167.2 | win (cpu_s) |
| pcbench-Raspberry-Pi-Soft-Power-Controller_Zero Current Soft Power | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.6 | | 0 | 322.0 | baseline |
| pcbench-Raspberry-Pi-Soft-Power-Controller_Zero Current Soft Power | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.6 | +0.0 | 0 | 322.0 | loss (cpu_s) |
| pcbench-ReSDMAC_ReSDMAC | kicad | main | 0.00 | 124 | 0 | 373.7 | | unmeasured | 298.5 | | 113 | 1228.5 | baseline |
| pcbench-ReSDMAC_ReSDMAC | kicad | neckdown | 0.00 | 113 | 0 | 429.3 | +55.6 | | 298.8 | +0.3 | 100 | 1213.5 | win (unrouted) |
| pcbench-ReST32_ReST RRD-FGC-Adapter | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 13.0 | | 9 | 721.4 | baseline |
| pcbench-ReST32_ReST RRD-FGC-Adapter | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 13.2 | +0.2 | 9 | 721.4 | loss (cpu_s) |
| pcbench-ReST32_ReST SD-Module | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 5.8 | | 5 | 551.1 | baseline |
| pcbench-ReST32_ReST SD-Module | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 5.9 | +0.0 | 5 | 551.1 | loss (cpu_s) |
| pcbench-Retro1DecodingModules_AddressDecoderModule | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 3.3 | | 0 | 529.3 | baseline |
| pcbench-Retro1DecodingModules_AddressDecoderModule | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 3.2 | -0.1 | 0 | 529.3 | win (cpu_s) |
| pcbench-RetroWiFiModem_RetroWiFiModem | kicad | main | 0.00 | 0 | 1 | 997.0 | | unmeasured | 37.0 | | 29 | 1519.4 | baseline |
| pcbench-RetroWiFiModem_RetroWiFiModem | kicad | neckdown | 0.00 | 0 | 1 | 997.0 | 0.0 | | 37.4 | +0.4 | 29 | 1519.4 | loss (cpu_s) |
| pcbench-RoBoC_CameraAdaptor | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 3.5 | | 0 | 701.3 | baseline |
| pcbench-RoBoC_CameraAdaptor | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 3.6 | +0.1 | 0 | 701.3 | loss (cpu_s) |
| pcbench-RoBoC_RoboticsMKII | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 252.6 | | 82 | 3505.8 | baseline |
| pcbench-RoBoC_RoboticsMKII | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 215.6 | -36.9 | 82 | 3505.8 | win (cpu_s) |
| pcbench-S1G-Mod_JST_Adapter | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.1 | | 0 | 59.9 | baseline |
| pcbench-S1G-Mod_JST_Adapter | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.1 | 0.0 | 0 | 59.9 | loss (peak_rss_mb) |
| pcbench-S4A-Mini-board_s4a-mini-board | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 4.0 | | 7 | 1126.8 | baseline |
| pcbench-S4A-Mini-board_s4a-mini-board | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 4.0 | -0.1 | 7 | 1126.8 | win (cpu_s) |
| pcbench-SMDBreakouts_smd_breakout | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.9 | | 0 | 294.4 | baseline |
| pcbench-SMDBreakouts_smd_breakout | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.9 | +0.0 | 0 | 294.4 | loss (cpu_s) |
| pcbench-SMDBreakouts_smd_breakout_quad | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.9 | | 0 | 294.4 | baseline |
| pcbench-SMDBreakouts_smd_breakout_quad | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.0 | +0.1 | 0 | 294.4 | loss (cpu_s) |
| pcbench-SNAP-Badge_SNAP_badge | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 187.1 | | 141 | 4762.6 | baseline |
| pcbench-SNAP-Badge_SNAP_badge | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 203.7 | +16.6 | 136 | 5110.3 | loss (score) |
| pcbench-SOICbite_SOICbite_SWD | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.8 | | 3 | 199.3 | baseline |
| pcbench-SOICbite_SOICbite_SWD | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.9 | +0.1 | 3 | 199.3 | loss (cpu_s) |
| pcbench-STM32F303_LQFP48_STM32_LQFP48 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 113.3 | | 23 | 1067.3 | baseline |
| pcbench-STM32F303_LQFP48_STM32_LQFP48 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 105.7 | -7.6 | 31 | 1019.5 | loss (score) |
| pcbench-STM32F373_LQFP48_STM32_LQFP48 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 71.3 | | 22 | 1005.4 | baseline |
| pcbench-STM32F373_LQFP48_STM32_LQFP48 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 89.8 | +18.5 | 24 | 989.7 | loss (score) |
| pcbench-Shift-in-32-HC165_shift-in | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 24.0 | | 0 | 1526.8 | baseline |
| pcbench-Shift-in-32-HC165_shift-in | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 24.3 | +0.3 | 0 | 1526.8 | loss (cpu_s) |
| pcbench-Shift-out-32-HC595_Shift-out | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 38.9 | | 37 | 1409.5 | baseline |
| pcbench-Shift-out-32-HC595_Shift-out | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 39.7 | +0.8 | 37 | 1409.5 | loss (cpu_s) |
| pcbench-SimpleCPLD_SimpleCPLD | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 48.0 | | 30 | 773.9 | baseline |
| pcbench-SimpleCPLD_SimpleCPLD | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 48.6 | +0.6 | 30 | 773.9 | loss (cpu_s) |
| pcbench-SmartLaserCO2-PCB_LaserPointer | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.0 | | 0 | 49.8 | baseline |
| pcbench-SmartLaserCO2-PCB_LaserPointer | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.0 | 0.0 | 0 | 49.8 | loss (peak_rss_mb) |
| pcbench-SmartLaserCO2-PCB_OptAdjust | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.0 | | 0 | 79.3 | baseline |
| pcbench-SmartLaserCO2-PCB_OptAdjust | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.0 | -0.0 | 0 | 79.3 | win (cpu_s) |
| pcbench-SmartLaserCO2-PCB_WaterCool | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.1 | | 0 | 106.3 | baseline |
| pcbench-SmartLaserCO2-PCB_WaterCool | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.1 | 0.0 | 0 | 106.3 | tie (peak_rss_mb) |
| pcbench-Solare-BQ24210_Solare-BQ24210 | kicad | main | 0.00 | 1 | 0 | 916.7 | | unmeasured | 69.2 | | 0 | 134.7 | baseline |
| pcbench-Solare-BQ24210_Solare-BQ24210 | kicad | neckdown | 0.00 | 1 | 0 | 916.7 | 0.0 | | 68.9 | -0.3 | 0 | 134.7 | win (cpu_s) |
| pcbench-SparkSwitch_SparkProtectionSwitch | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.7 | | 0 | 204.4 | baseline |
| pcbench-SparkSwitch_SparkProtectionSwitch | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.7 | +0.0 | 0 | 204.4 | loss (cpu_s) |
| pcbench-Starburst-One_alpha | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.4 | | 2 | 95.7 | baseline |
| pcbench-Starburst-One_alpha | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.5 | +0.0 | 2 | 95.7 | loss (cpu_s) |
| pcbench-Starling_Starling_V1 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 68.1 | | 27 | 1350.8 | baseline |
| pcbench-Starling_Starling_V1 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 68.2 | +0.1 | 27 | 1350.8 | loss (cpu_s) |
| pcbench-Starling__autosave-Starling WiPSU ver_0.1 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 3.9 | | 1 | 400.3 | baseline |
| pcbench-Starling__autosave-Starling WiPSU ver_0.1 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 4.0 | +0.1 | 1 | 400.3 | loss (cpu_s) |
| pcbench-SynthDrumTrigger_Synth Drum Trigger | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.5 | | 0 | 437.5 | baseline |
| pcbench-SynthDrumTrigger_Synth Drum Trigger | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.5 | +0.0 | 0 | 437.5 | loss (cpu_s) |
| pcbench-TB6600StepperDriver_DEW_TB6600-V1 | kicad | main | 0.00 | 6 | 0 | 896.5 | | unmeasured | 147.4 | | 41 | 2245.4 | baseline |
| pcbench-TB6600StepperDriver_DEW_TB6600-V1 | kicad | neckdown | 0.00 | 6 | 0 | 896.5 | 0.0 | | 143.5 | -3.8 | 41 | 2245.4 | win (cpu_s) |
| pcbench-TK44_TK44 | kicad | main | 0.00 | 15 | 1 | 838.3 | | unmeasured | 175.1 | | 39 | 3221.3 | baseline |
| pcbench-TK44_TK44 | kicad | neckdown | 0.00 | 15 | 1 | 838.3 | 0.0 | | 169.6 | -5.6 | 39 | 3221.3 | win (cpu_s) |
| pcbench-TLPHnodeV2_TLPHnodeV2 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 41.7 | | 8 | 483.4 | baseline |
| pcbench-TLPHnodeV2_TLPHnodeV2 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 42.5 | +0.8 | 8 | 483.4 | loss (cpu_s) |
| pcbench-TMC261-stepstick_TMC261-stepstick-v1.1 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 81.4 | | 62 | 3076.0 | baseline |
| pcbench-TMC261-stepstick_TMC261-stepstick-v1.1 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 81.5 | +0.1 | 62 | 3076.0 | loss (cpu_s) |
| pcbench-TX5823_TX5823 | kicad | main | 0.00 | 3 | 2 | 970.7 | | unmeasured | 153.4 | | 73 | 1607.5 | baseline |
| pcbench-TX5823_TX5823 | kicad | neckdown | 0.00 | 3 | 2 | 970.7 | 0.0 | | 148.5 | -4.9 | 73 | 1607.5 | win (cpu_s) |
| pcbench-Teensy-3.5-Breakout-Boaard_PropShield_Uno | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 3.4 | | 4 | 1115.5 | baseline |
| pcbench-Teensy-3.5-Breakout-Boaard_PropShield_Uno | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 3.5 | +0.1 | 4 | 1115.5 | loss (cpu_s) |
| pcbench-Teensy-3.5-Breakout-Boaard_TeensyMegaIoShield | kicad | main | 0.00 | 2 | 0 | 976.2 | | unmeasured | 168.4 | | 40 | 3734.5 | baseline |
| pcbench-Teensy-3.5-Breakout-Boaard_TeensyMegaIoShield | kicad | neckdown | 0.00 | 2 | 0 | 976.2 | 0.0 | | 161.2 | -7.2 | 40 | 3734.5 | win (cpu_s) |
| pcbench-Teensy-3.5-Breakout-Boaard_Test | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.2 | | 0 | 843.3 | baseline |
| pcbench-Teensy-3.5-Breakout-Boaard_Test | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.2 | +0.0 | 0 | 843.3 | loss (cpu_s) |
| pcbench-Teensy-Hats_Teensy-7-Segment-Hat | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.8 | | 3 | 176.4 | baseline |
| pcbench-Teensy-Hats_Teensy-7-Segment-Hat | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.8 | +0.0 | 3 | 176.4 | loss (cpu_s) |
| pcbench-Teensy-Hats_Teensy-LCD-LiDAR-Hat | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.9 | | 2 | 765.2 | baseline |
| pcbench-Teensy-Hats_Teensy-LCD-LiDAR-Hat | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 3.0 | +0.1 | 2 | 765.2 | loss (cpu_s) |
| pcbench-TeensyProtoboard_TeensyProtoboard | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 4.6 | | 2 | 535.7 | baseline |
| pcbench-TeensyProtoboard_TeensyProtoboard | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 4.6 | +0.0 | 2 | 535.7 | loss (cpu_s) |
| pcbench-ThinkerShield_ThinkerShield | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 5.8 | | 7 | 991.6 | baseline |
| pcbench-ThinkerShield_ThinkerShield | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 5.9 | +0.1 | 7 | 991.6 | loss (cpu_s) |
| pcbench-TinyTracker_ub-minimal | kicad | main | 0.00 | 4 | 0 | 904.8 | | unmeasured | 109.1 | | 37 | 612.8 | baseline |
| pcbench-TinyTracker_ub-minimal | kicad | neckdown | 0.00 | 4 | 0 | 904.8 | 0.0 | | 110.3 | +1.2 | 37 | 612.8 | loss (cpu_s) |
| pcbench-ToslinkCNC_Toslink PlanetCNC ECO shield | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.9 | | 0 | 1056.4 | baseline |
| pcbench-ToslinkCNC_Toslink PlanetCNC ECO shield | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.9 | +0.0 | 0 | 1056.4 | loss (cpu_s) |
| pcbench-ToslinkCNC__autosave-ToslinkCNC_OneAxis | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 38.3 | | 30 | 1283.8 | baseline |
| pcbench-ToslinkCNC__autosave-ToslinkCNC_OneAxis | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 38.7 | +0.4 | 30 | 1283.8 | loss (cpu_s) |
| pcbench-ToslinkCNC_toslink_arduino_shield | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 26.1 | | 17 | 1876.8 | baseline |
| pcbench-ToslinkCNC_toslink_arduino_shield | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 26.1 | -0.0 | 17 | 1876.8 | win (cpu_s) |
| pcbench-TripleDelay2399_TripleDelay2399 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 15.7 | | 0 | 2154.8 | baseline |
| pcbench-TripleDelay2399_TripleDelay2399 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 16.1 | +0.4 | 0 | 2154.8 | loss (cpu_s) |
| pcbench-ULPI-Pmod_ULPI-Pmod | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 27.9 | | 13 | 561.3 | baseline |
| pcbench-ULPI-Pmod_ULPI-Pmod | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 27.8 | -0.1 | 12 | 588.3 | win (score) |
| pcbench-UProgrammer-Hardware_Programmer | kicad | main | 0.00 | 2 | 0 | 969.2 | | unmeasured | 105.9 | | 84 | 2032.4 | baseline |
| pcbench-UProgrammer-Hardware_Programmer | kicad | neckdown | 0.00 | 2 | 0 | 969.2 | 0.0 | | 106.7 | +0.8 | 84 | 2032.4 | loss (cpu_s) |
| pcbench-UltraPIF_Hardware_led | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.3 | | 3 | 39.0 | baseline |
| pcbench-UltraPIF_Hardware_led | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.3 | 0.0 | 3 | 39.0 | tie (peak_rss_mb) |
| pcbench-UltraPIF_Hardware_pif_adapter | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 3.9 | | 6 | 229.4 | baseline |
| pcbench-UltraPIF_Hardware_pif_adapter | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 4.0 | +0.1 | 6 | 229.4 | loss (cpu_s) |
| pcbench-UltrasonicSystem_Schematic | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 28.6 | | 10 | 3567.8 | baseline |
| pcbench-UltrasonicSystem_Schematic | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 29.4 | +0.8 | 10 | 3567.8 | loss (cpu_s) |
| pcbench-UniversalBoard4Nucleo_Nucleo_Universal_Board | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 7.3 | | 0 | 1253.2 | baseline |
| pcbench-UniversalBoard4Nucleo_Nucleo_Universal_Board | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 7.6 | +0.3 | 0 | 1253.2 | loss (cpu_s) |
| pcbench-Usb-Serial-Breakout-Cp2102_cp2102 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 9.7 | | 6 | 163.1 | baseline |
| pcbench-Usb-Serial-Breakout-Cp2102_cp2102 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 9.7 | -0.1 | 6 | 163.1 | win (cpu_s) |
| pcbench-VC4000MultiROM_MultiRomCard | kicad | main | 0.00 | 2 | 13 | 964.9 | | unmeasured | 123.0 | | 48 | 6716.9 | baseline |
| pcbench-VC4000MultiROM_MultiRomCard | kicad | neckdown | 0.00 | 2 | 13 | 964.9 | 0.0 | | 123.3 | +0.3 | 48 | 6716.9 | loss (cpu_s) |
| pcbench-VM-sensor-PT1000_vm-sensor-pt100 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 6.7 | | 3 | 390.7 | baseline |
| pcbench-VM-sensor-PT1000_vm-sensor-pt100 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 6.7 | 0.0 | 3 | 390.7 | win (peak_rss_mb) |
| pcbench-Ventilator_indicator-board | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.0 | | 2 | 139.7 | baseline |
| pcbench-Ventilator_indicator-board | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.0 | -0.0 | 2 | 139.7 | win (cpu_s) |
| pcbench-Ventilator_pressure_XGZP6897A | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.2 | | 2 | 152.6 | baseline |
| pcbench-Ventilator_pressure_XGZP6897A | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.2 | -0.0 | 2 | 152.6 | win (cpu_s) |
| pcbench-Ventilator_pressure_mpx5700ap_gp | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.2 | | 2 | 134.0 | baseline |
| pcbench-Ventilator_pressure_mpx5700ap_gp | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.2 | +0.1 | 2 | 134.0 | loss (cpu_s) |
| pcbench-Ventilator_pressure_mpxv5004_10dp | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.2 | | 3 | 132.2 | baseline |
| pcbench-Ventilator_pressure_mpxv5004_10dp | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.2 | +0.0 | 3 | 132.2 | loss (cpu_s) |
| pcbench-Ventilator_pressure_mpxv5004dp | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.1 | | 3 | 132.2 | baseline |
| pcbench-Ventilator_pressure_mpxv5004dp | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.2 | +0.1 | 3 | 132.2 | loss (cpu_s) |
| pcbench-Ventilator_pressure_mpxv5010dp | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.1 | | 3 | 132.2 | baseline |
| pcbench-Ventilator_pressure_mpxv5010dp | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.2 | +0.1 | 3 | 132.2 | loss (cpu_s) |
| pcbench-Ventilator_pressure_ms4525do | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.5 | | 2 | 131.1 | baseline |
| pcbench-Ventilator_pressure_ms4525do | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.5 | +0.0 | 2 | 131.1 | loss (cpu_s) |
| pcbench-WHCS-Base-Station_base-station | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 57.0 | | 32 | 2933.7 | baseline |
| pcbench-WHCS-Base-Station_base-station | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 57.8 | +0.8 | 32 | 2933.7 | loss (cpu_s) |
| pcbench-WS2811LEDMatrix_matrixcontrol | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 47.6 | | 32 | 1442.2 | baseline |
| pcbench-WS2811LEDMatrix_matrixcontrol | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 48.0 | +0.4 | 32 | 1442.2 | loss (cpu_s) |
| pcbench-WeatherSpot_vreg_pressure | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.1 | | 0 | 59.9 | baseline |
| pcbench-WeatherSpot_vreg_pressure | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.2 | +0.0 | 0 | 59.9 | loss (cpu_s) |
| pcbench-WordClock_v2.0_WordClock_v2 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 5.8 | | 5 | 552.3 | baseline |
| pcbench-WordClock_v2.0_WordClock_v2 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 5.9 | +0.1 | 5 | 552.3 | loss (cpu_s) |
| pcbench-Youyue-858D-plus-MCU-adapter_youyue-858d-plus-mcu-adapter | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 4.5 | | 1 | 426.7 | baseline |
| pcbench-Youyue-858D-plus-MCU-adapter_youyue-858d-plus-mcu-adapter | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 4.7 | +0.2 | 1 | 426.7 | loss (cpu_s) |
| pcbench-abus-cfa1000-display-grabber_acs-display-grabber | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 44.1 | | 85 | 3482.7 | baseline |
| pcbench-abus-cfa1000-display-grabber_acs-display-grabber | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 45.0 | +0.9 | 85 | 3482.7 | loss (cpu_s) |
| pcbench-aciduino_aciduino_pcb | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 13.2 | | 3 | 1969.5 | baseline |
| pcbench-aciduino_aciduino_pcb | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 13.4 | +0.1 | 3 | 1969.5 | loss (cpu_s) |
| pcbench-airqualitystation_hardware | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 9.5 | | 12 | 663.4 | baseline |
| pcbench-airqualitystation_hardware | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 9.8 | +0.3 | 12 | 663.4 | loss (cpu_s) |
| pcbench-akuhei_akuhei | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 13.4 | | 7 | 320.2 | baseline |
| pcbench-akuhei_akuhei | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 13.7 | +0.3 | 7 | 320.2 | loss (cpu_s) |
| pcbench-alu_gate_xnor_2in | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 7.5 | | 0 | 418.3 | baseline |
| pcbench-alu_gate_xnor_2in | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 7.6 | +0.2 | 0 | 418.3 | loss (cpu_s) |
| pcbench-amalthea_amalthea_rev0 | kicad | main | 0.00 | 23 | 0 | 923.3 | | unmeasured | 298.8 | | 233 | 1958.2 | baseline |
| pcbench-amalthea_amalthea_rev0 | kicad | neckdown | 0.00 | 23 | 0 | 923.3 | 0.0 | | 299.8 | +1.0 | 233 | 1958.2 | loss (cpu_s) |
| pcbench-analog_esr_meter_esr_meter_rev_a | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 4.8 | | 0 | 803.3 | baseline |
| pcbench-analog_esr_meter_esr_meter_rev_a | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 4.9 | +0.1 | 0 | 803.3 | loss (cpu_s) |
| pcbench-anima_MotorDrive | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 41.0 | | 21 | 1532.5 | baseline |
| pcbench-anima_MotorDrive | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 42.0 | +1.0 | 21 | 1532.5 | loss (cpu_s) |
| pcbench-antdroid-board_antdroid-board | kicad | main | 0.00 | 18 | 0 | 806.4 | | unmeasured | 68.2 | | 0 | 1527.8 | baseline |
| pcbench-antdroid-board_antdroid-board | kicad | neckdown | 0.00 | 18 | 0 | 806.4 | 0.0 | | 68.5 | +0.3 | 0 | 1527.8 | loss (cpu_s) |
| pcbench-apa102lantern_apa102-lantern-side | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 4.4 | | 0 | 591.4 | baseline |
| pcbench-apa102lantern_apa102-lantern-side | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 4.5 | +0.1 | 0 | 591.4 | loss (cpu_s) |
| pcbench-arduino-led-driver_arduino-led-driver | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 192.1 | | 58 | 2501.9 | baseline |
| pcbench-arduino-led-driver_arduino-led-driver | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 176.2 | -15.9 | 58 | 2501.9 | win (cpu_s) |
| pcbench-arduino_arduino leds | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.4 | | 0 | 734.1 | baseline |
| pcbench-arduino_arduino leds | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.4 | +0.0 | 0 | 734.1 | loss (cpu_s) |
| pcbench-atmegax8-protoboard_atmegax8-protoboard | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.5 | | 0 | 344.6 | baseline |
| pcbench-atmegax8-protoboard_atmegax8-protoboard | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.5 | +0.0 | 0 | 344.6 | loss (cpu_s) |
| pcbench-atmel-programmer_atmel_programmer | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.7 | | 0 | 923.0 | baseline |
| pcbench-atmel-programmer_atmel_programmer | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.8 | +0.1 | 0 | 923.0 | loss (cpu_s) |
| pcbench-audio_relay_input_switch_relay_switch | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.8 | | 2 | 574.7 | baseline |
| pcbench-audio_relay_input_switch_relay_switch | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.9 | +0.0 | 2 | 574.7 | loss (cpu_s) |
| pcbench-audprog_audprog_v2 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 14.4 | | 9 | 733.2 | baseline |
| pcbench-audprog_audprog_v2 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 14.8 | +0.4 | 9 | 733.2 | loss (cpu_s) |
| pcbench-autohat-board_inverted-usd-adapter | kicad | main | 0.00 | 0 | 7 | 825.0 | | unmeasured | 0.1 | | 0 | 240.1 | baseline |
| pcbench-autohat-board_inverted-usd-adapter | kicad | neckdown | 0.00 | 0 | 7 | 825.0 | 0.0 | | 0.1 | 0.0 | 0 | 240.1 | loss (peak_rss_mb) |
| pcbench-autohat-board_usd-adapter | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.4 | | 5 | 269.1 | baseline |
| pcbench-autohat-board_usd-adapter | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.5 | +0.0 | 5 | 269.1 | loss (cpu_s) |
| pcbench-avr-fuser-32_adapter | kicad | main | 0.00 | 0 | 24 | 759.9 | | unmeasured | 176.4 | | 65 | 6086.1 | baseline |
| pcbench-avr-fuser-32_adapter | kicad | neckdown | 0.00 | 0 | 24 | 759.9 | 0.0 | | 169.3 | -7.1 | 65 | 6086.1 | win (cpu_s) |
| pcbench-avr_ledprojector_avr_ledprojection | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 17.5 | | 43 | 1145.8 | baseline |
| pcbench-avr_ledprojector_avr_ledprojection | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 17.3 | -0.2 | 43 | 1145.8 | win (cpu_s) |
| pcbench-avr_ledprojector_avr_ledprojection-0402 | kicad | main | 0.00 | 1 | 4 | 982.7 | | unmeasured | 110.5 | | 57 | 812.7 | baseline |
| pcbench-avr_ledprojector_avr_ledprojection-0402 | kicad | neckdown | 0.00 | 1 | 6 | 978.8 | -3.8 | | 155.5 | +45.1 | 55 | 844.6 | loss (violations) |
| pcbench-azalea_azalea | kicad | main | 0.00 | 45 | 0 | 666.7 | | unmeasured | 282.6 | | 148 | 3276.6 | baseline |
| pcbench-azalea_azalea | kicad | neckdown | 0.00 | 45 | 0 | 666.7 | 0.0 | | 232.3 | -50.3 | 148 | 3276.6 | win (cpu_s) |
| pcbench-badge2016_Badge_init | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 3.0 | | 3 | 441.3 | baseline |
| pcbench-badge2016_Badge_init | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 3.0 | +0.1 | 3 | 441.3 | loss (cpu_s) |
| pcbench-balena-rover-wide-hat_resin-rover | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 129.6 | | 24 | 2219.8 | baseline |
| pcbench-balena-rover-wide-hat_resin-rover | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 130.4 | +0.9 | 24 | 2219.8 | loss (cpu_s) |
| pcbench-basic_esp_board_basic_esp_board | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 24.6 | | 13 | 1272.1 | baseline |
| pcbench-basic_esp_board_basic_esp_board | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 24.7 | +0.1 | 13 | 1272.1 | loss (cpu_s) |
| pcbench-beast-phat_beast-phat | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.8 | | 0 | 356.3 | baseline |
| pcbench-beast-phat_beast-phat | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.9 | +0.1 | 0 | 356.3 | loss (cpu_s) |
| pcbench-bee-light-measurement-matrix_bee-light-measurement-matrix | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 20.5 | | 22 | 2313.6 | baseline |
| pcbench-bee-light-measurement-matrix_bee-light-measurement-matrix | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 20.6 | +0.1 | 22 | 2313.6 | loss (cpu_s) |
| pcbench-beer-gauge_sensorboard | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 4.3 | | 0 | 217.0 | baseline |
| pcbench-beer-gauge_sensorboard | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 4.5 | +0.1 | 0 | 217.0 | loss (cpu_s) |
| pcbench-beryl_rain_beryl_rain | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 6.7 | | 8 | 533.4 | baseline |
| pcbench-beryl_rain_beryl_rain | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 6.8 | +0.1 | 8 | 533.4 | loss (cpu_s) |
| pcbench-beyblock20_beyblock20 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 41.0 | | 26 | 1685.7 | baseline |
| pcbench-beyblock20_beyblock20 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 42.6 | +1.6 | 26 | 1685.7 | loss (cpu_s) |
| pcbench-bikedar_bikedar | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 48.0 | | 13 | 773.5 | baseline |
| pcbench-bikedar_bikedar | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 48.8 | +0.8 | 13 | 773.5 | loss (cpu_s) |
| pcbench-blackmagic-isolated_mmp | kicad | main | 0.00 | 1 | 0 | 982.8 | | unmeasured | 76.8 | | 35 | 620.0 | baseline |
| pcbench-blackmagic-isolated_mmp | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | +17.2 | | 85.3 | +8.5 | 37 | 626.7 | win (clean_pass_rate) |
| pcbench-bldc-gimbal-1d_gimbal-board | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 21.2 | | 26 | 901.4 | baseline |
| pcbench-bldc-gimbal-1d_gimbal-board | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 21.1 | -0.1 | 26 | 901.4 | win (cpu_s) |
| pcbench-blinky-badge_blinky | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 8.7 | | 4 | 539.5 | baseline |
| pcbench-blinky-badge_blinky | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 8.9 | +0.3 | 4 | 539.5 | loss (cpu_s) |
| pcbench-bms-8s50-ic_bms-8s50-ic | kicad | main | 0.00 | 55 | 0 | 615.4 | | unmeasured | 298.2 | | 111 | 2869.7 | baseline |
| pcbench-bms-8s50-ic_bms-8s50-ic | kicad | neckdown | 0.00 | 40 | 10 | 706.3 | +90.9 | | 299.8 | +1.6 | 130 | 3126.8 | win (unrouted) |
| pcbench-bmw-ibus-bluetooth_bmw_bt_cdcemu_analog | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 7.4 | | 0 | 390.0 | baseline |
| pcbench-bmw-ibus-bluetooth_bmw_bt_cdcemu_analog | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 7.5 | +0.1 | 0 | 390.0 | loss (cpu_s) |
| pcbench-bmw-ibus-bluetooth_bmw_bt_cdcemu_digital | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 25.2 | | 10 | 970.7 | baseline |
| pcbench-bmw-ibus-bluetooth_bmw_bt_cdcemu_digital | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 25.4 | +0.3 | 10 | 970.7 | loss (cpu_s) |
| pcbench-board_armjtag_pmod_compatible_armjtag-pmod | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.3 | | 0 | 274.8 | baseline |
| pcbench-board_armjtag_pmod_compatible_armjtag-pmod | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.3 | -0.0 | 0 | 274.8 | win (cpu_s) |
| pcbench-boards_shift-register-demo-v2 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 3.8 | | 0 | 1143.3 | baseline |
| pcbench-boards_shift-register-demo-v2 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 4.0 | +0.2 | 0 | 1143.3 | loss (cpu_s) |
| pcbench-boatcontrol_CommonCathode60A | kicad | main | 0.00 | 93 | 0 | 0.0 | | unmeasured | 9.3 | | 4 | 1064.9 | baseline |
| pcbench-boatcontrol_CommonCathode60A | kicad | neckdown | 0.00 | 93 | 0 | 0.0 | 0.0 | | 9.4 | +0.1 | 4 | 1064.9 | loss (cpu_s) |
| pcbench-boatcontrol_NonLatchingNO30A | kicad | main | 0.00 | 27 | 0 | 449.0 | | unmeasured | 106.4 | | 21 | 2768.6 | baseline |
| pcbench-boatcontrol_NonLatchingNO30A | kicad | neckdown | 0.00 | 27 | 0 | 449.0 | 0.0 | | 106.4 | -0.0 | 21 | 2768.6 | win (cpu_s) |
| pcbench-bobc_LCD-panel-adapter-lvc | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 12.5 | | 12 | 864.6 | baseline |
| pcbench-bobc_LCD-panel-adapter-lvc | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 12.3 | -0.1 | 12 | 864.6 | win (cpu_s) |
| pcbench-bobc_MS-F100 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 93.1 | | 30 | 1019.9 | baseline |
| pcbench-bobc_MS-F100 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 94.2 | +1.1 | 30 | 1019.9 | loss (cpu_s) |
| pcbench-bobc_led_clock | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 90.3 | | 47 | 2441.7 | baseline |
| pcbench-bobc_led_clock | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 90.9 | +0.6 | 47 | 2441.7 | loss (cpu_s) |
| pcbench-bobc_matrix_clock | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 194.8 | | 109 | 5167.0 | baseline |
| pcbench-bobc_matrix_clock | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 170.2 | -24.6 | 109 | 5167.0 | win (cpu_s) |
| pcbench-bpnode-bb_BPnode-BB | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 5.9 | | 8 | 422.2 | baseline |
| pcbench-bpnode-bb_BPnode-BB | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 5.7 | -0.2 | 8 | 422.2 | win (cpu_s) |
| pcbench-breakout-boards_50-to-100 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.7 | | 4 | 109.2 | baseline |
| pcbench-breakout-boards_50-to-100 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.7 | +0.0 | 4 | 109.2 | loss (cpu_s) |
| pcbench-breakout-boards_avr-isp-x2 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.0 | | 0 | 42.2 | baseline |
| pcbench-breakout-boards_avr-isp-x2 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.0 | 0.0 | 0 | 42.2 | tie (peak_rss_mb) |
| pcbench-breakout-boards_esp8266-jtag | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 4.8 | | 8 | 400.5 | baseline |
| pcbench-breakout-boards_esp8266-jtag | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 4.9 | +0.1 | 8 | 400.5 | loss (cpu_s) |
| pcbench-breakout-boards_swd-and-uart | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.4 | | 2 | 112.5 | baseline |
| pcbench-breakout-boards_swd-and-uart | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.4 | -0.0 | 2 | 112.5 | win (cpu_s) |
| pcbench-breakout-boards_swd-to-wires | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.5 | | 0 | 105.0 | baseline |
| pcbench-breakout-boards_swd-to-wires | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.5 | 0.0 | 0 | 105.0 | tie (peak_rss_mb) |
| pcbench-bristle_bot_light_follow_bristle_bot | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.6 | | 0 | 350.1 | baseline |
| pcbench-bristle_bot_light_follow_bristle_bot | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.6 | -0.0 | 0 | 350.1 | win (cpu_s) |
| pcbench-busblaster-to-swd_busblaster-to-swd | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.6 | | 0 | 240.7 | baseline |
| pcbench-busblaster-to-swd_busblaster-to-swd | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.6 | +0.0 | 0 | 240.7 | loss (cpu_s) |
| pcbench-bypass_crossmix_bypass_crossmix | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 14.7 | | 17 | 1338.4 | baseline |
| pcbench-bypass_crossmix_bypass_crossmix | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 14.8 | +0.0 | 17 | 1338.4 | loss (cpu_s) |
| pcbench-can_firewall_hardware_CAN_Firewall | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 179.1 | | 88 | 2236.0 | baseline |
| pcbench-can_firewall_hardware_CAN_Firewall | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 156.8 | -22.3 | 97 | 2255.1 | loss (score) |
| pcbench-cdm324_backpack_cdm324 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 4.1 | | 3 | 252.0 | baseline |
| pcbench-cdm324_backpack_cdm324 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 4.2 | +0.0 | 3 | 252.0 | loss (cpu_s) |
| pcbench-ciurlys_ciurlys | kicad | main | 0.00 | 5 | 0 | 583.3 | | unmeasured | 46.9 | | 5 | 243.6 | baseline |
| pcbench-ciurlys_ciurlys | kicad | neckdown | 0.00 | 3 | 0 | 750.0 | +166.7 | | 47.5 | +0.6 | 10 | 230.1 | win (unrouted) |
| pcbench-clock_lcdb4 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 18.3 | | 7 | 1352.8 | baseline |
| pcbench-clock_lcdb4 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 18.2 | -0.1 | 7 | 1352.8 | win (cpu_s) |
| pcbench-cnlohr_wiflier | kicad | main | 0.00 | 17 | 0 | 552.6 | | unmeasured | 125.8 | | 48 | 716.1 | baseline |
| pcbench-cnlohr_wiflier | kicad | neckdown | 0.00 | 17 | 0 | 552.6 | 0.0 | | 123.6 | -2.2 | 48 | 716.1 | win (cpu_s) |
| pcbench-cnlohr_wiflier_B | kicad | main | 0.00 | 16 | 0 | 600.0 | | unmeasured | 128.2 | | 45 | 685.1 | baseline |
| pcbench-cnlohr_wiflier_B | kicad | neckdown | 0.00 | 16 | 0 | 600.0 | 0.0 | | 128.5 | +0.3 | 45 | 685.1 | loss (cpu_s) |
| pcbench-continuity-tester_continuity-tester | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.7 | | 2 | 223.1 | baseline |
| pcbench-continuity-tester_continuity-tester | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.7 | 0.0 | 2 | 223.1 | tie (peak_rss_mb) |
| pcbench-cookiecutter-xsproduct_{{cookiecutter.product_name}} | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.3 | | 0 | 173.1 | baseline |
| pcbench-cookiecutter-xsproduct_{{cookiecutter.product_name}} | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.2 | -0.0 | 0 | 173.1 | win (cpu_s) |
| pcbench-crossover-schiit-stack_xover4schiit | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.2 | | 0 | 174.0 | baseline |
| pcbench-crossover-schiit-stack_xover4schiit | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.2 | -0.0 | 0 | 174.0 | win (cpu_s) |
| pcbench-custom_cpu--ALU_custom_cpu--ALU | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 10.3 | | 5 | 1962.7 | baseline |
| pcbench-custom_cpu--ALU_custom_cpu--ALU | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 10.4 | +0.1 | 5 | 1962.7 | loss (cpu_s) |
| pcbench-custom_cpu--register_custom_cpu--register | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.3 | | 0 | 1353.8 | baseline |
| pcbench-custom_cpu--register_custom_cpu--register | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.4 | +0.0 | 0 | 1353.8 | loss (cpu_s) |
| pcbench-data-manager_data-manager | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 35.9 | | 33 | 1215.3 | baseline |
| pcbench-data-manager_data-manager | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 36.6 | +0.8 | 33 | 1215.3 | loss (cpu_s) |
| pcbench-decelerator4030_decelerator4030 | kicad | main | 0.00 | 476 | 10 | 0.0 | | unmeasured | 299.0 | | 598 | 7142.4 | baseline |
| pcbench-decelerator4030_decelerator4030 | kicad | neckdown | 0.00 | 496 | 11 | 0.0 | 0.0 | | 299.9 | +0.9 | 597 | 7305.0 | loss (unrouted) |
| pcbench-denbit_basic | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.7 | | 0 | 624.4 | baseline |
| pcbench-denbit_basic | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.7 | 0.0 | 0 | 624.4 | loss (peak_rss_mb) |
| pcbench-deskbot_breakout | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.5 | | 1 | 347.7 | baseline |
| pcbench-deskbot_breakout | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.5 | +0.0 | 1 | 347.7 | loss (cpu_s) |
| pcbench-devttys0_IRis | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 4.2 | | 4 | 295.6 | baseline |
| pcbench-devttys0_IRis | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 4.3 | +0.0 | 4 | 295.6 | loss (cpu_s) |
| pcbench-digital_clock_led_clock_3_and_4_digit | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 24.7 | | 26 | 4848.3 | baseline |
| pcbench-digital_clock_led_clock_3_and_4_digit | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 24.7 | +0.0 | 26 | 4848.3 | loss (cpu_s) |
| pcbench-digital_clock_led_clock_v1 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 23.8 | | 27 | 5499.3 | baseline |
| pcbench-digital_clock_led_clock_v1 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 24.1 | +0.3 | 27 | 5499.3 | loss (cpu_s) |
| pcbench-disco-dongle_DiscoDongle | kicad | main | 0.00 | 0 | 1 | 993.3 | | unmeasured | 10.3 | | 10 | 459.8 | baseline |
| pcbench-disco-dongle_DiscoDongle | kicad | neckdown | 0.00 | 0 | 1 | 993.3 | 0.0 | | 10.4 | +0.1 | 10 | 459.8 | loss (cpu_s) |
| pcbench-divergence_meter_dm_control | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 66.5 | | 41 | 1878.3 | baseline |
| pcbench-divergence_meter_dm_control | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 72.9 | +6.4 | 42 | 1877.0 | loss (score) |
| pcbench-domotics_base-board-arranged | kicad | main | 0.00 | 0 | 3 | 996.9 | | unmeasured | 45.6 | | 55 | 14623.8 | baseline |
| pcbench-domotics_base-board-arranged | kicad | neckdown | 0.00 | 0 | 3 | 996.9 | 0.0 | | 46.3 | +0.7 | 55 | 14623.8 | loss (cpu_s) |
| pcbench-dorkyboard_keyboard | kicad | main | 0.00 | 4 | 0 | 972.6 | | unmeasured | 205.5 | | 64 | 13594.3 | baseline |
| pcbench-dorkyboard_keyboard | kicad | neckdown | 0.00 | 4 | 0 | 972.6 | 0.0 | | 180.9 | -24.6 | 64 | 13594.3 | win (cpu_s) |
| pcbench-drawduino_drawduino | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.6 | | 0 | 212.8 | baseline |
| pcbench-drawduino_drawduino | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.6 | +0.0 | 0 | 212.8 | loss (cpu_s) |
| pcbench-dust_sensor_dust_sensor | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 34.2 | | 23 | 1480.2 | baseline |
| pcbench-dust_sensor_dust_sensor | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 34.4 | +0.1 | 23 | 1480.2 | loss (cpu_s) |
| pcbench-dustbox_Dustbox | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.4 | | 0 | 257.0 | baseline |
| pcbench-dustbox_Dustbox | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.4 | -0.0 | 0 | 257.0 | win (cpu_s) |
| pcbench-eBUS-Adapter_Groeger | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.3 | | 0 | 250.3 | baseline |
| pcbench-eBUS-Adapter_Groeger | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.3 | -0.0 | 0 | 250.3 | win (cpu_s) |
| pcbench-eeg_brainboard_batteryv0 | kicad | main | 0.00 | 1 | 0 | 972.2 | | unmeasured | 105.7 | | 29 | 1210.0 | baseline |
| pcbench-eeg_brainboard_batteryv0 | kicad | neckdown | 0.00 | 1 | 0 | 972.2 | -0.0 | | 101.4 | -4.2 | 30 | 1180.1 | loss (score) |
| pcbench-eink-adapter_eink | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 184.8 | | 51 | 2634.1 | baseline |
| pcbench-eink-adapter_eink | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 133.3 | -51.5 | 71 | 2600.8 | loss (score) |
| pcbench-epaper-102_epaper-102 | kicad | main | 0.00 | 6 | 0 | 793.1 | | unmeasured | 76.6 | | 40 | 485.0 | baseline |
| pcbench-epaper-102_epaper-102 | kicad | neckdown | 0.00 | 6 | 0 | 793.1 | 0.0 | | 76.2 | -0.4 | 40 | 485.0 | win (cpu_s) |
| pcbench-epapercard_epapercard | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 77.3 | | 36 | 1919.2 | baseline |
| pcbench-epapercard_epapercard | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 77.4 | +0.1 | 36 | 1919.2 | loss (cpu_s) |
| pcbench-ergo-snm-keyboard_receiver | kicad | main | 0.00 | 10 | 0 | 833.3 | | unmeasured | 51.8 | | 13 | 254.9 | baseline |
| pcbench-ergo-snm-keyboard_receiver | kicad | neckdown | 0.00 | 10 | 0 | 833.3 | 0.0 | | 52.2 | +0.4 | 13 | 254.9 | loss (cpu_s) |
| pcbench-esp-leipa_esp-12 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 3.0 | | 4 | 424.1 | baseline |
| pcbench-esp-leipa_esp-12 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.9 | -0.1 | 4 | 424.1 | win (cpu_s) |
| pcbench-esp-serial-terminal_esp-com | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 6.4 | | 6 | 572.1 | baseline |
| pcbench-esp-serial-terminal_esp-com | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 6.5 | +0.1 | 6 | 572.1 | loss (cpu_s) |
| pcbench-esp12-appliance_mod_esp12-appliance-mod | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 3.9 | | 5 | 751.2 | baseline |
| pcbench-esp12-appliance_mod_esp12-appliance-mod | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 3.9 | +0.0 | 5 | 751.2 | loss (cpu_s) |
| pcbench-esp12-breakout_ESP12Breakout | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.9 | | 0 | 205.0 | baseline |
| pcbench-esp12-breakout_ESP12Breakout | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.9 | +0.1 | 0 | 205.0 | loss (cpu_s) |
| pcbench-esp32-4-channel-relays_esp32-4-channel-relays | kicad | main | 0.00 | 7 | 0 | 922.2 | | unmeasured | 20.1 | | 27 | 806.1 | baseline |
| pcbench-esp32-4-channel-relays_esp32-4-channel-relays | kicad | neckdown | 0.00 | 7 | 0 | 922.2 | 0.0 | | 20.3 | +0.2 | 27 | 806.1 | loss (cpu_s) |
| pcbench-esp32-ethernet_esp32-ethernet | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 143.5 | | 36 | 1435.3 | baseline |
| pcbench-esp32-ethernet_esp32-ethernet | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 133.0 | -10.5 | 36 | 1435.3 | win (cpu_s) |
| pcbench-esp32stack_esp32stack | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 14.3 | | 26 | 1071.1 | baseline |
| pcbench-esp32stack_esp32stack | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 14.4 | +0.1 | 26 | 1071.1 | loss (cpu_s) |
| pcbench-esp8266-for-uppatvind_Air-Purifier-Uppatvind | kicad | main | 0.00 | 13 | 0 | 551.7 | | unmeasured | 60.7 | | 5 | 201.9 | baseline |
| pcbench-esp8266-for-uppatvind_Air-Purifier-Uppatvind | kicad | neckdown | 0.00 | 13 | 0 | 551.7 | 0.0 | | 60.6 | -0.1 | 5 | 201.9 | win (cpu_s) |
| pcbench-esp8266_32x32panel_esp_12_f_595 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 145.8 | | 41 | 1658.5 | baseline |
| pcbench-esp8266_32x32panel_esp_12_f_595 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 135.5 | -10.3 | 41 | 1658.5 | win (cpu_s) |
| pcbench-esp8266_32x32panel_esp_12_f_595_ORDERED | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 145.8 | | 41 | 1658.5 | baseline |
| pcbench-esp8266_32x32panel_esp_12_f_595_ORDERED | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 131.7 | -14.1 | 41 | 1658.5 | win (cpu_s) |
| pcbench-esp8266_envmonitor_environment-monitor | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.4 | | 2 | 366.2 | baseline |
| pcbench-esp8266_envmonitor_environment-monitor | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.4 | +0.0 | 2 | 366.2 | loss (cpu_s) |
| pcbench-esp8266_envmonitor_environment-monitor-1.2 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.9 | | 1 | 412.9 | baseline |
| pcbench-esp8266_envmonitor_environment-monitor-1.2 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.0 | +0.0 | 1 | 412.9 | loss (cpu_s) |
| pcbench-esp8266_envmonitor_environment-monitor-1.4 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.4 | | 2 | 366.2 | baseline |
| pcbench-esp8266_envmonitor_environment-monitor-1.4 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.4 | 0.0 | 2 | 366.2 | tie (peak_rss_mb) |
| pcbench-esp8266_link_test_esp_micro85-only | kicad | main | 0.00 | 3 | 0 | 769.2 | | unmeasured | 30.4 | | 2 | 96.2 | baseline |
| pcbench-esp8266_link_test_esp_micro85-only | kicad | neckdown | 0.00 | 3 | 0 | 769.2 | -0.0 | | 30.4 | -0.1 | 2 | 96.6 | loss (score) |
| pcbench-esp8266_network_speaker_esp_network_speaker | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 20.2 | | 21 | 625.8 | baseline |
| pcbench-esp8266_network_speaker_esp_network_speaker | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 20.7 | +0.4 | 21 | 625.8 | loss (cpu_s) |
| pcbench-esp8266_wi07_3_adapter_esp | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.4 | | 2 | 114.8 | baseline |
| pcbench-esp8266_wi07_3_adapter_esp | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.4 | 0.0 | 2 | 114.8 | loss (peak_rss_mb) |
| pcbench-espalarm_alarm | kicad | main | 0.00 | 0 | 73 | 496.5 | | unmeasured | 31.9 | | 17 | 1950.6 | baseline |
| pcbench-espalarm_alarm | kicad | neckdown | 0.00 | 0 | 73 | 496.5 | 0.0 | | 32.3 | +0.5 | 17 | 1950.6 | loss (cpu_s) |
| pcbench-esper_EsperDNS | kicad | main | 0.00 | 9 | 0 | 910.9 | | unmeasured | 72.3 | | 50 | 965.1 | baseline |
| pcbench-esper_EsperDNS | kicad | neckdown | 0.00 | 9 | 0 | 910.9 | 0.0 | | 73.1 | +0.8 | 50 | 965.1 | loss (cpu_s) |
| pcbench-esper_programmer | kicad | main | 0.00 | 12 | 0 | 777.8 | | unmeasured | 16.2 | | 16 | 353.3 | baseline |
| pcbench-esper_programmer | kicad | neckdown | 0.00 | 12 | 0 | 777.8 | 0.0 | | 16.1 | -0.1 | 16 | 353.3 | win (cpu_s) |
| pcbench-espeverywhere__autosave-espeverywhere_breakout | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.2 | | 0 | 135.6 | baseline |
| pcbench-espeverywhere__autosave-espeverywhere_breakout | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.2 | +0.0 | 0 | 135.6 | loss (cpu_s) |
| pcbench-espionage_esplight | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.1 | | 7 | 416.3 | baseline |
| pcbench-espionage_esplight | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.1 | +0.0 | 7 | 416.3 | loss (cpu_s) |
| pcbench-everled_everled | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.3 | | 2 | 141.5 | baseline |
| pcbench-everled_everled | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.3 | -0.0 | 2 | 141.5 | win (cpu_s) |
| pcbench-ezusb-logicanalyzer_cypress_logic_analyzer | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.0 | | 3 | 485.1 | baseline |
| pcbench-ezusb-logicanalyzer_cypress_logic_analyzer | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.0 | +0.0 | 3 | 485.1 | loss (cpu_s) |
| pcbench-f.60_keyboard | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 13.6 | | 0 | 4057.6 | baseline |
| pcbench-f.60_keyboard | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 13.7 | +0.0 | 0 | 4057.6 | loss (cpu_s) |
| pcbench-fan_controller_fan_controller | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 10.2 | | 17 | 881.5 | baseline |
| pcbench-fan_controller_fan_controller | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 10.1 | -0.1 | 17 | 881.5 | win (cpu_s) |
| pcbench-fifogfx_c64cart | kicad | main | 0.00 | 2 | 0 | 954.5 | | unmeasured | 159.0 | | 26 | 2769.5 | baseline |
| pcbench-fifogfx_c64cart | kicad | neckdown | 0.00 | 2 | 0 | 954.5 | 0.0 | | 138.3 | -20.7 | 26 | 2769.5 | win (cpu_s) |
| pcbench-filament_extruder_sensor | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.9 | | 4 | 367.8 | baseline |
| pcbench-filament_extruder_sensor | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.9 | -0.0 | 4 | 367.8 | win (cpu_s) |
| pcbench-fingerprint-with-esp32_quet van tay | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.0 | | 2 | 487.4 | baseline |
| pcbench-fingerprint-with-esp32_quet van tay | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.0 | +0.0 | 2 | 487.4 | loss (cpu_s) |
| pcbench-firefly-jar_solar_lamp | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.5 | | 0 | 274.6 | baseline |
| pcbench-firefly-jar_solar_lamp | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.5 | +0.0 | 0 | 274.6 | loss (cpu_s) |
| pcbench-fp2_extension_sample_fp2_usb_breakout | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.2 | | 1 | 63.8 | baseline |
| pcbench-fp2_extension_sample_fp2_usb_breakout | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.2 | 0.0 | 1 | 63.8 | tie (peak_rss_mb) |
| pcbench-free-of-charge_BMS | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 167.0 | | 72 | 2046.9 | baseline |
| pcbench-free-of-charge_BMS | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 152.5 | -14.5 | 72 | 2046.9 | win (cpu_s) |
| pcbench-freeDSP-CLASSIC-SMD-BALANCED_FreeDSP_BAL | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 159.4 | | 68 | 4131.3 | baseline |
| pcbench-freeDSP-CLASSIC-SMD-BALANCED_FreeDSP_BAL | kicad | neckdown | 0.00 | 2 | 0 | 986.5 | -13.5 | | 100.6 | -58.9 | 64 | 4178.9 | loss (clean_pass_rate) |
| pcbench-freeUSBi_USBi_Programmer | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 3.0 | | 1 | 574.8 | baseline |
| pcbench-freeUSBi_USBi_Programmer | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 3.0 | -0.0 | 1 | 574.8 | win (cpu_s) |
| pcbench-ftdi-jtag-programmer_JTAGProgrammer | kicad | main | 0.00 | 12 | 2 | 866.7 | | unmeasured | 198.7 | | 106 | 1198.2 | baseline |
| pcbench-ftdi-jtag-programmer_JTAGProgrammer | kicad | neckdown | 0.00 | 5 | 2 | 941.9 | +75.3 | | 170.1 | -28.6 | 105 | 1192.0 | win (unrouted) |
| pcbench-gb-hardware_GB-BRK-M-XS | kicad | main | 0.00 | 0 | 2 | 987.9 | | unmeasured | 11.9 | | 26 | 545.8 | baseline |
| pcbench-gb-hardware_GB-BRK-M-XS | kicad | neckdown | 0.00 | 0 | 2 | 987.9 | 0.0 | | 11.9 | +0.0 | 26 | 545.8 | loss (cpu_s) |
| pcbench-gb-hardware_GB-BRK-TR-A | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.0 | | 0 | 68.7 | baseline |
| pcbench-gb-hardware_GB-BRK-TR-A | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.0 | 0.0 | 0 | 68.7 | tie (peak_rss_mb) |
| pcbench-gb-hardware_GB-CART256K-A | kicad | main | 0.00 | 8 | 0 | 813.9 | | unmeasured | 134.6 | | 94 | 1791.7 | baseline |
| pcbench-gb-hardware_GB-CART256K-A | kicad | neckdown | 0.00 | 6 | 0 | 860.4 | +46.5 | | 148.3 | +13.7 | 96 | 1815.4 | win (unrouted) |
| pcbench-gb-hardware_GB-CART32K-A | kicad | main | 0.00 | 1 | 0 | 964.3 | | unmeasured | 17.5 | | 46 | 1176.9 | baseline |
| pcbench-gb-hardware_GB-CART32K-A | kicad | neckdown | 0.00 | 1 | 0 | 964.3 | 0.0 | | 17.6 | +0.1 | 46 | 1176.9 | loss (cpu_s) |
| pcbench-gb-hardware_GB-CARTPP-XC | kicad | main | 0.00 | 9 | 0 | 800.0 | | unmeasured | 143.4 | | 61 | 1129.5 | baseline |
| pcbench-gb-hardware_GB-CARTPP-XC | kicad | neckdown | 0.00 | 9 | 0 | 800.0 | 0.0 | | 129.1 | -14.3 | 61 | 1129.5 | win (cpu_s) |
| pcbench-gb-hardware_GB-LIVE32 | kicad | main | 0.00 | 5 | 0 | 941.2 | | unmeasured | 227.9 | | 127 | 2342.4 | baseline |
| pcbench-gb-hardware_GB-LIVE32 | kicad | neckdown | 0.00 | 5 | 0 | 941.2 | 0.0 | | 187.1 | -40.8 | 127 | 2342.4 | win (cpu_s) |
| pcbench-gb-hardware_GB-MBCTEST | kicad | main | 0.00 | 3 | 0 | 967.7 | | unmeasured | 216.3 | | 127 | 2800.3 | baseline |
| pcbench-gb-hardware_GB-MBCTEST | kicad | neckdown | 0.00 | 3 | 0 | 967.7 | 0.0 | | 186.6 | -29.7 | 127 | 2800.3 | win (cpu_s) |
| pcbench-gdrom_adapter_board_adapter | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 124.5 | | 40 | 3361.3 | baseline |
| pcbench-gdrom_adapter_board_adapter | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 115.0 | -9.6 | 40 | 3361.3 | win (cpu_s) |
| pcbench-gepetto_circuito | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 4.0 | | 0 | 966.4 | baseline |
| pcbench-gepetto_circuito | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 4.1 | +0.1 | 0 | 966.4 | loss (cpu_s) |
| pcbench-guitar_fret | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 64.3 | | 19 | 991.3 | baseline |
| pcbench-guitar_fret | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 65.8 | +1.5 | 19 | 991.3 | loss (cpu_s) |
| pcbench-gwurrbus_pwm | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.1 | | 0 | 1054.4 | baseline |
| pcbench-gwurrbus_pwm | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.1 | -0.0 | 0 | 1054.4 | win (cpu_s) |
| pcbench-hackaday_esp-14_power_meter__autosave-esp-14 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.2 | | 0 | 163.9 | baseline |
| pcbench-hackaday_esp-14_power_meter__autosave-esp-14 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.3 | +0.1 | 0 | 163.9 | loss (cpu_s) |
| pcbench-hackpad_orpheuspad_pcb | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 3.6 | | 4 | 1188.8 | baseline |
| pcbench-hackpad_orpheuspad_pcb | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 3.6 | +0.0 | 4 | 1188.8 | loss (cpu_s) |
| pcbench-hackyflasher_Flasher | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.2 | | 0 | 663.9 | baseline |
| pcbench-hackyflasher_Flasher | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.2 | 0.0 | 0 | 663.9 | loss (peak_rss_mb) |
| pcbench-hardware-designs_c-trigger | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.1 | | 0 | 84.5 | baseline |
| pcbench-hardware-designs_c-trigger | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.1 | 0.0 | 0 | 84.5 | tie (peak_rss_mb) |
| pcbench-hardware-designs_m-trigger | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.1 | | 0 | 105.3 | baseline |
| pcbench-hardware-designs_m-trigger | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.1 | 0.0 | 0 | 105.3 | tie (peak_rss_mb) |
| pcbench-hardware-designs_nixie-combo | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 16.1 | | 5 | 2794.3 | baseline |
| pcbench-hardware-designs_nixie-combo | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 16.2 | +0.1 | 5 | 2794.3 | loss (cpu_s) |
| pcbench-hardware-designs_nixie-power | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.3 | | 1 | 485.3 | baseline |
| pcbench-hardware-designs_nixie-power | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.3 | +0.0 | 1 | 485.3 | loss (cpu_s) |
| pcbench-hardware-designs_soil-moisture-sensor | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.0 | | 4 | 466.9 | baseline |
| pcbench-hardware-designs_soil-moisture-sensor | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.0 | 0.0 | 4 | 466.9 | tie (peak_rss_mb) |
| pcbench-hardware-designs_solar-harvester | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.6 | | 3 | 227.0 | baseline |
| pcbench-hardware-designs_solar-harvester | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.6 | 0.0 | 3 | 227.0 | win (peak_rss_mb) |
| pcbench-hardware-designs_spsgrf-board | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.6 | | 5 | 182.3 | baseline |
| pcbench-hardware-designs_spsgrf-board | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.6 | 0.0 | 5 | 182.3 | win (peak_rss_mb) |
| pcbench-hbr-mk2_hbr-mk2-bpfs | kicad | main | 0.00 | 7 | 0 | 915.7 | | unmeasured | 135.7 | | 38 | 2069.8 | baseline |
| pcbench-hbr-mk2_hbr-mk2-bpfs | kicad | neckdown | 0.00 | 7 | 0 | 915.7 | 0.0 | | 126.8 | -8.9 | 38 | 2069.8 | win (cpu_s) |
| pcbench-hbr-mk2_hbr-mk2-digital | kicad | main | 0.00 | 14 | 0 | 818.2 | | unmeasured | 37.3 | | 23 | 2044.1 | baseline |
| pcbench-hbr-mk2_hbr-mk2-digital | kicad | neckdown | 0.00 | 14 | 0 | 818.2 | 0.0 | | 38.0 | +0.6 | 23 | 2044.1 | loss (cpu_s) |
| pcbench-hbr-mk2_hbr-mk2-lpfs | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 21.1 | | 17 | 1249.4 | baseline |
| pcbench-hbr-mk2_hbr-mk2-lpfs | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 21.5 | +0.4 | 17 | 1249.4 | loss (cpu_s) |
| pcbench-headstage-adapter_headstage adapter | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 136.9 | | 108 | 1886.1 | baseline |
| pcbench-headstage-adapter_headstage adapter | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 132.2 | -4.7 | 108 | 1886.1 | win (cpu_s) |
| pcbench-helmholtz-servo_CurrentServo | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 60.3 | | 10 | 2713.9 | baseline |
| pcbench-helmholtz-servo_CurrentServo | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 60.2 | -0.1 | 10 | 2713.9 | win (cpu_s) |
| pcbench-hm-mod-rpi-rtc_hm-mod-rpi-rtc | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.5 | | 4 | 354.8 | baseline |
| pcbench-hm-mod-rpi-rtc_hm-mod-rpi-rtc | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.4 | -0.1 | 4 | 354.8 | win (cpu_s) |
| pcbench-hw_trials_demo | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 5.5 | | 11 | 1168.3 | baseline |
| pcbench-hw_trials_demo | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 5.7 | +0.2 | 11 | 1168.3 | loss (cpu_s) |
| pcbench-hwstar_ac-power-monitor | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 30.7 | | 15 | 1222.3 | baseline |
| pcbench-hwstar_ac-power-monitor | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 30.0 | -0.7 | 15 | 1222.3 | win (cpu_s) |
| pcbench-icehat_icehat | kicad | main | 0.00 | 1 | 0 | 986.5 | | unmeasured | 188.0 | | 47 | 1402.6 | baseline |
| pcbench-icehat_icehat | kicad | neckdown | 0.00 | 1 | 0 | 986.5 | 0.0 | | 159.6 | -28.4 | 47 | 1402.6 | win (cpu_s) |
| pcbench-imfr-schematics_Telescopio | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.6 | | 0 | 1347.0 | baseline |
| pcbench-imfr-schematics_Telescopio | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.5 | -0.1 | 0 | 1347.0 | win (cpu_s) |
| pcbench-induction-hob_temperature-sender | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 15.0 | | 22 | 686.5 | baseline |
| pcbench-induction-hob_temperature-sender | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 15.1 | +0.0 | 22 | 686.5 | loss (cpu_s) |
| pcbench-jadonk_PocketBone | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 214.3 | | 79 | 1626.7 | baseline |
| pcbench-jadonk_PocketBone | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 184.5 | -29.8 | 79 | 1626.7 | win (cpu_s) |
| pcbench-jdy-08-board_jdy-08 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.8 | | 1 | 422.3 | baseline |
| pcbench-jdy-08-board_jdy-08 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.8 | -0.0 | 1 | 422.3 | win (cpu_s) |
| pcbench-juno-chorus-clone_juno-chorus-clone | kicad | main | 0.00 | 0 | 2 | 996.3 | | unmeasured | 83.5 | | 24 | 4429.5 | baseline |
| pcbench-juno-chorus-clone_juno-chorus-clone | kicad | neckdown | 0.00 | 0 | 2 | 996.3 | 0.0 | | 82.9 | -0.6 | 24 | 4429.5 | win (cpu_s) |
| pcbench-karabas-nano_karabas-nano-revA | kicad | main | 0.00 | 33 | 0 | 777.0 | | unmeasured | 299.3 | | 326 | 8319.6 | baseline |
| pcbench-karabas-nano_karabas-nano-revA | kicad | neckdown | 0.00 | 30 | 0 | 797.3 | +20.3 | | 300.3 | +1.0 | 333 | 8637.2 | win (unrouted) |
| pcbench-karabas-nano_karabas-nano-revB | kicad | main | 0.00 | 77 | 0 | 506.4 | | unmeasured | 298.9 | | 332 | 6685.3 | baseline |
| pcbench-karabas-nano_karabas-nano-revB | kicad | neckdown | 0.00 | 59 | 0 | 621.8 | +115.4 | | 300.1 | +1.2 | 334 | 7463.1 | win (unrouted) |
| pcbench-karabas-nano_karabas-nano-revC | kicad | main | 0.00 | 97 | 1 | 485.7 | | unmeasured | 298.8 | | 311 | 7064.8 | baseline |
| pcbench-karabas-nano_karabas-nano-revC | kicad | neckdown | 0.00 | 68 | 1 | 639.1 | +153.4 | | 300.0 | +1.2 | 319 | 7867.5 | win (unrouted) |
| pcbench-karabas-nano_karabas-nano-revG | kicad | main | 0.00 | 86 | 0 | 582.5 | | unmeasured | 298.8 | | 323 | 7029.6 | baseline |
| pcbench-karabas-nano_karabas-nano-revG | kicad | neckdown | 0.00 | 71 | 0 | 655.3 | +72.8 | | 300.2 | +1.4 | 347 | 7730.0 | win (unrouted) |
| pcbench-karabas-nano_wifi_revA | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.4 | | 3 | 280.7 | baseline |
| pcbench-karabas-nano_wifi_revA | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.4 | +0.0 | 3 | 280.7 | loss (cpu_s) |
| pcbench-kassenautomat.mdb-interface_mdb-interface | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 51.8 | | 24 | 2225.2 | baseline |
| pcbench-kassenautomat.mdb-interface_mdb-interface | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 52.2 | +0.4 | 24 | 2225.2 | loss (cpu_s) |
| pcbench-keyboards_Djinn | kicad | main | 0.00 | 386 | 0 | 0.0 | | unmeasured | 309.7 | | 137 | 2195.6 | baseline |
| pcbench-keyboards_Djinn | kicad | neckdown | 0.00 | 386 | 0 | 0.0 | 0.0 | | 303.7 | -6.0 | 137 | 2195.6 | win (cpu_s) |
| pcbench-kicad-guitar-preamp_Preamp-Instructables | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.2 | | 0 | 146.2 | baseline |
| pcbench-kicad-guitar-preamp_Preamp-Instructables | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.3 | +0.0 | 0 | 146.2 | loss (cpu_s) |
| pcbench-kicad-projects_BatCharge | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.9 | | 4 | 136.5 | baseline |
| pcbench-kicad-projects_BatCharge | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.0 | +0.0 | 4 | 136.5 | loss (cpu_s) |
| pcbench-kicad-projects_ili9341-breakout | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.8 | | 0 | 294.7 | baseline |
| pcbench-kicad-projects_ili9341-breakout | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.8 | -0.0 | 0 | 294.7 | win (cpu_s) |
| pcbench-kicad_bbb-melzi | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.9 | | 0 | 404.2 | baseline |
| pcbench-kicad_bbb-melzi | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.0 | +0.0 | 0 | 404.2 | loss (cpu_s) |
| pcbench-kika-in-space_DS8500 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.4 | | 1 | 271.8 | baseline |
| pcbench-kika-in-space_DS8500 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.4 | 0.0 | 1 | 271.8 | loss (peak_rss_mb) |
| pcbench-kika-in-space_analog-test-board | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.9 | | 0 | 157.1 | baseline |
| pcbench-kika-in-space_analog-test-board | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.9 | +0.0 | 0 | 157.1 | loss (cpu_s) |
| pcbench-kinetoscope_ethernet | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.1 | | 0 | 158.9 | baseline |
| pcbench-kinetoscope_ethernet | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.1 | 0.0 | 0 | 158.9 | tie (peak_rss_mb) |
| pcbench-kinetoscope_microcontroller | kicad | main | 0.00 | 49 | 0 | 666.7 | | unmeasured | 159.7 | | 99 | 3215.1 | baseline |
| pcbench-kinetoscope_microcontroller | kicad | neckdown | 0.00 | 49 | 0 | 666.7 | 0.0 | | 139.7 | -20.0 | 99 | 3215.1 | win (cpu_s) |
| pcbench-kinetoscope_sram-bank | kicad | main | 0.00 | 103 | 0 | 331.2 | | unmeasured | 298.5 | | 193 | 4445.7 | baseline |
| pcbench-kinetoscope_sram-bank | kicad | neckdown | 0.00 | 105 | 0 | 318.2 | -13.0 | | 299.4 | +0.9 | 190 | 4323.1 | loss (unrouted) |
| pcbench-kit2-led-cube_led_cube | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 21.3 | | 10 | 936.6 | baseline |
| pcbench-kit2-led-cube_led_cube | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 22.2 | +0.9 | 10 | 936.6 | loss (cpu_s) |
| pcbench-kitspace_12V5A_breakout | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.7 | | 0 | 216.5 | baseline |
| pcbench-kitspace_12V5A_breakout | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.7 | 0.0 | 0 | 216.5 | tie (peak_rss_mb) |
| pcbench-kitspace_12_24_boost_converter | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.9 | | 0 | 305.0 | baseline |
| pcbench-kitspace_12_24_boost_converter | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.9 | +0.0 | 0 | 305.0 | loss (cpu_s) |
| pcbench-kitspace_40-channel-hv-switching-board | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 298.4 | | 254 | 7970.4 | baseline |
| pcbench-kitspace_40-channel-hv-switching-board | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 299.9 | +1.4 | 251 | 8008.6 | win (score) |
| pcbench-kitspace_4_switch_array | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.2 | | 0 | 303.0 | baseline |
| pcbench-kitspace_4_switch_array | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.2 | 0.0 | 0 | 303.0 | win (peak_rss_mb) |
| pcbench-kitspace_8_switch_array | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.8 | | 0 | 581.7 | baseline |
| pcbench-kitspace_8_switch_array | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.8 | +0.0 | 0 | 581.7 | loss (cpu_s) |
| pcbench-kitspace_BQ25570_Harvester | kicad | main | 0.00 | 2 | 0 | 846.1 | | unmeasured | 48.9 | | 9 | 182.8 | baseline |
| pcbench-kitspace_BQ25570_Harvester | kicad | neckdown | 0.00 | 2 | 0 | 846.1 | 0.0 | | 49.0 | +0.1 | 9 | 182.8 | loss (cpu_s) |
| pcbench-kitspace_CH330 | kicad | main | 0.00 | 1 | 0 | 909.1 | | unmeasured | 36.9 | | 4 | 72.0 | baseline |
| pcbench-kitspace_CH330 | kicad | neckdown | 0.00 | 1 | 0 | 909.1 | 0.0 | | 37.6 | +0.7 | 4 | 72.0 | loss (cpu_s) |
| pcbench-kitspace_CO2 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 18.1 | | 13 | 789.4 | baseline |
| pcbench-kitspace_CO2 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 18.5 | +0.4 | 13 | 789.4 | loss (cpu_s) |
| pcbench-kitspace_DIY_detector | kicad | main | 0.00 | 0 | 1 | 983.3 | | unmeasured | 4.9 | | 0 | 390.0 | baseline |
| pcbench-kitspace_DIY_detector | kicad | neckdown | 0.00 | 0 | 1 | 983.3 | 0.0 | | 4.7 | -0.2 | 0 | 390.0 | win (cpu_s) |
| pcbench-kitspace_Lcr_addon | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 3.4 | | 0 | 839.6 | baseline |
| pcbench-kitspace_Lcr_addon | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 3.4 | 0.0 | 0 | 839.6 | loss (peak_rss_mb) |
| pcbench-kitspace_Minisumo_V2.1 | kicad | main | 0.00 | 1 | 0 | 969.7 | | unmeasured | 59.3 | | 21 | 1045.1 | baseline |
| pcbench-kitspace_Minisumo_V2.1 | kicad | neckdown | 0.00 | 1 | 0 | 969.7 | 0.0 | | 58.7 | -0.6 | 21 | 1045.1 | win (cpu_s) |
| pcbench-kitspace_OSO-BOOK-C1 | kicad | main | 0.00 | 7 | 32 | 673.1 | | unmeasured | 217.6 | | 71 | 2070.1 | baseline |
| pcbench-kitspace_OSO-BOOK-C1 | kicad | neckdown | 0.00 | 5 | 32 | 721.9 | +48.8 | | 90.0 | -127.6 | 52 | 2270.8 | win (unrouted) |
| pcbench-kitspace_OtterScreen | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 37.4 | | 27 | 737.2 | baseline |
| pcbench-kitspace_OtterScreen | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 37.9 | +0.5 | 27 | 737.2 | loss (cpu_s) |
| pcbench-kitspace_PSLab | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 196.7 | | 113 | 3232.6 | baseline |
| pcbench-kitspace_PSLab | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 169.2 | -27.5 | 113 | 3232.6 | win (cpu_s) |
| pcbench-kitspace_Potentiometer_mount_4LED | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.1 | | 0 | 204.5 | baseline |
| pcbench-kitspace_Potentiometer_mount_4LED | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.1 | 0.0 | 0 | 204.5 | win (peak_rss_mb) |
| pcbench-kitspace_Potentiometer_mount_8LED | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.3 | | 0 | 313.2 | baseline |
| pcbench-kitspace_Potentiometer_mount_8LED | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.3 | +0.0 | 0 | 313.2 | loss (cpu_s) |
| pcbench-kitspace_RPi_shield | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.7 | | 0 | 333.2 | baseline |
| pcbench-kitspace_RPi_shield | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.7 | 0.0 | 0 | 333.2 | tie (peak_rss_mb) |
| pcbench-kitspace_T32_ref | kicad | main | 0.00 | 0 | 1 | 997.2 | | unmeasured | 298.4 | | 63 | 2012.6 | baseline |
| pcbench-kitspace_T32_ref | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | +2.7 | | 299.5 | +1.0 | 63 | 2030.2 | win (clean_pass_rate) |
| pcbench-kitspace_USB-C-Screen-Adapter | kicad | main | 0.00 | 2 | 0 | 973.3 | | unmeasured | 186.0 | | 79 | 1112.2 | baseline |
| pcbench-kitspace_USB-C-Screen-Adapter | kicad | neckdown | 0.00 | 2 | 0 | 973.3 | 0.0 | | 155.7 | -30.3 | 79 | 1112.2 | win (cpu_s) |
| pcbench-kitspace_USB-C-Screen-Adapter-LDR6023SS | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 99.2 | | 36 | 887.9 | baseline |
| pcbench-kitspace_USB-C-Screen-Adapter-LDR6023SS | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 91.9 | -7.2 | 36 | 887.9 | win (cpu_s) |
| pcbench-kitspace__autosave-nunchuk_breakout | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.4 | | 0 | 153.9 | baseline |
| pcbench-kitspace__autosave-nunchuk_breakout | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.3 | -0.0 | 0 | 153.9 | win (cpu_s) |
| pcbench-kitspace_aquarius | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 54.8 | | 50 | 2399.8 | baseline |
| pcbench-kitspace_aquarius | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 55.0 | +0.2 | 50 | 2399.8 | loss (cpu_s) |
| pcbench-kitspace_ardfpga | kicad | main | 0.00 | 2 | 0 | 976.2 | | unmeasured | 156.5 | | 103 | 2604.8 | baseline |
| pcbench-kitspace_ardfpga | kicad | neckdown | 0.00 | 2 | 0 | 976.2 | 0.0 | | 144.2 | -12.4 | 103 | 2604.8 | win (cpu_s) |
| pcbench-kitspace_beehive | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 5.7 | | 0 | 1637.6 | baseline |
| pcbench-kitspace_beehive | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 5.8 | +0.0 | 0 | 1637.6 | loss (cpu_s) |
| pcbench-kitspace_dropbot-front-panel | kicad | main | 0.00 | 24 | 0 | 832.1 | | unmeasured | 298.5 | | 193 | 7102.6 | baseline |
| pcbench-kitspace_dropbot-front-panel | kicad | neckdown | 0.00 | 17 | 0 | 881.1 | +49.0 | | 299.7 | +1.2 | 199 | 7530.6 | win (unrouted) |
| pcbench-kitspace_dropbot_control_board | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 77.0 | | 57 | 3213.7 | baseline |
| pcbench-kitspace_dropbot_control_board | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 78.3 | +1.2 | 57 | 3213.7 | loss (cpu_s) |
| pcbench-kitspace_dynamixel_shield | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 6.5 | | 3 | 912.3 | baseline |
| pcbench-kitspace_dynamixel_shield | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 6.5 | +0.0 | 3 | 912.3 | loss (cpu_s) |
| pcbench-kitspace_esp8266 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 6.0 | | 0 | 605.5 | baseline |
| pcbench-kitspace_esp8266 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 6.1 | +0.1 | 0 | 605.5 | loss (cpu_s) |
| pcbench-kitspace_flypi | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 37.2 | | 27 | 2566.4 | baseline |
| pcbench-kitspace_flypi | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 38.1 | +0.8 | 27 | 2566.4 | loss (cpu_s) |
| pcbench-kitspace_flypi_v2 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 15.5 | | 0 | 1857.7 | baseline |
| pcbench-kitspace_flypi_v2 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 15.4 | -0.1 | 0 | 1857.7 | win (cpu_s) |
| pcbench-kitspace_gas_sensor | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.6 | | 0 | 232.0 | baseline |
| pcbench-kitspace_gas_sensor | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.6 | 0.0 | 0 | 232.0 | tie (peak_rss_mb) |
| pcbench-kitspace_grove_adaptor | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.0 | | 0 | 17.7 | baseline |
| pcbench-kitspace_grove_adaptor | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.0 | -0.0 | 0 | 17.7 | win (cpu_s) |
| pcbench-kitspace_hbridge_driver | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.5 | | 0 | 570.5 | baseline |
| pcbench-kitspace_hbridge_driver | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.5 | +0.0 | 0 | 570.5 | loss (cpu_s) |
| pcbench-kitspace_hp_led_switch | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.1 | | 0 | 546.6 | baseline |
| pcbench-kitspace_hp_led_switch | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.1 | -0.0 | 0 | 546.6 | win (cpu_s) |
| pcbench-kitspace_hum_temp_sensor | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.1 | | 0 | 94.8 | baseline |
| pcbench-kitspace_hum_temp_sensor | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.1 | 0.0 | 0 | 94.8 | tie (peak_rss_mb) |
| pcbench-kitspace_ideal_diode | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 3.4 | | 3 | 188.1 | baseline |
| pcbench-kitspace_ideal_diode | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 3.5 | +0.1 | 3 | 188.1 | loss (cpu_s) |
| pcbench-kitspace_ir_sensor | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.1 | | 0 | 228.1 | baseline |
| pcbench-kitspace_ir_sensor | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.1 | +0.0 | 0 | 228.1 | loss (cpu_s) |
| pcbench-kitspace_led_driver | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 3.2 | | 1 | 600.8 | baseline |
| pcbench-kitspace_led_driver | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 3.3 | +0.1 | 1 | 600.8 | loss (cpu_s) |
| pcbench-kitspace_level_shifter | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.5 | | 0 | 453.6 | baseline |
| pcbench-kitspace_level_shifter | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.6 | +0.0 | 0 | 453.6 | loss (cpu_s) |
| pcbench-kitspace_minisumo_v3 | kicad | main | 0.00 | 1 | 0 | 982.1 | | unmeasured | 129.7 | | 64 | 2725.5 | baseline |
| pcbench-kitspace_minisumo_v3 | kicad | neckdown | 0.00 | 1 | 0 | 982.1 | 0.0 | | 112.1 | -17.6 | 64 | 2725.5 | win (cpu_s) |
| pcbench-kitspace_nunchuk_breakout | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.6 | | 0 | 154.8 | baseline |
| pcbench-kitspace_nunchuk_breakout | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.6 | 0.0 | 0 | 154.8 | win (peak_rss_mb) |
| pcbench-kitspace_peltier | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 3.0 | | 0 | 652.0 | baseline |
| pcbench-kitspace_peltier | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 3.1 | +0.1 | 0 | 652.0 | loss (cpu_s) |
| pcbench-kitspace_piezo_amplifier | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 19.0 | | 13 | 3811.7 | baseline |
| pcbench-kitspace_piezo_amplifier | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 19.2 | +0.2 | 13 | 3811.7 | loss (cpu_s) |
| pcbench-kitspace_pmt_combiner | kicad | main | 0.00 | 0 | 1 | 988.2 | | unmeasured | 0.5 | | 0 | 608.7 | baseline |
| pcbench-kitspace_pmt_combiner | kicad | neckdown | 0.00 | 0 | 1 | 988.2 | 0.0 | | 0.6 | +0.0 | 0 | 608.7 | loss (cpu_s) |
| pcbench-kitspace_power_supply | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.4 | | 0 | 296.5 | baseline |
| pcbench-kitspace_power_supply | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.4 | +0.0 | 0 | 296.5 | loss (cpu_s) |
| pcbench-kitspace_sensor | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 97.1 | | 27 | 2265.8 | baseline |
| pcbench-kitspace_sensor | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 91.8 | -5.3 | 27 | 2265.8 | win (cpu_s) |
| pcbench-kitspace_solenoid_driver | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.1 | | 0 | 347.8 | baseline |
| pcbench-kitspace_solenoid_driver | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.1 | 0.0 | 0 | 347.8 | tie (peak_rss_mb) |
| pcbench-kitspace_spike_n_hold | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.9 | | 0 | 1119.2 | baseline |
| pcbench-kitspace_spike_n_hold | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.9 | -0.0 | 0 | 1119.2 | win (cpu_s) |
| pcbench-kitspace_sympetrum-v2%20NFF1.1 | kicad | main | 0.00 | 13 | 0 | 847.1 | | unmeasured | 126.3 | | 28 | 835.8 | baseline |
| pcbench-kitspace_sympetrum-v2%20NFF1.1 | kicad | neckdown | 0.00 | 13 | 0 | 847.1 | 0.0 | | 110.3 | -16.0 | 28 | 835.8 | win (cpu_s) |
| pcbench-kitspace_teensy-fx | kicad | main | 0.00 | 2 | 0 | 977.8 | | unmeasured | 275.2 | | 146 | 3685.7 | baseline |
| pcbench-kitspace_teensy-fx | kicad | neckdown | 0.00 | 2 | 0 | 977.8 | 0.0 | | 214.8 | -60.4 | 146 | 3685.7 | win (cpu_s) |
| pcbench-kitspace_temp_breakout | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.2 | | 0 | 251.7 | baseline |
| pcbench-kitspace_temp_breakout | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.2 | -0.0 | 0 | 251.7 | win (cpu_s) |
| pcbench-kitspace_threeboard | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 22.0 | | 17 | 1245.0 | baseline |
| pcbench-kitspace_threeboard | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 22.2 | +0.2 | 17 | 1245.0 | loss (cpu_s) |
| pcbench-kitspace_training_board_v02 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 14.9 | | 0 | 1394.3 | baseline |
| pcbench-kitspace_training_board_v02 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 14.6 | -0.3 | 0 | 1394.3 | win (cpu_s) |
| pcbench-kitspace_trans_switch_volt_amp | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.7 | | 0 | 809.4 | baseline |
| pcbench-kitspace_trans_switch_volt_amp | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.7 | +0.0 | 0 | 809.4 | loss (cpu_s) |
| pcbench-kitspace_tt_nano_HAT_b1 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.8 | | 0 | 462.7 | baseline |
| pcbench-kitspace_tt_nano_HAT_b1 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.9 | +0.1 | 0 | 462.7 | loss (cpu_s) |
| pcbench-kitspace_tt_nano_HAT_b2 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 3.5 | | 0 | 654.7 | baseline |
| pcbench-kitspace_tt_nano_HAT_b2 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 3.3 | -0.2 | 0 | 654.7 | win (cpu_s) |
| pcbench-kitspace_tt_opt101_module_b1 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.1 | | 0 | 56.6 | baseline |
| pcbench-kitspace_tt_opt101_module_b1 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.1 | 0.0 | 0 | 56.6 | tie (peak_rss_mb) |
| pcbench-klangorium_logic_noise_playground | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 30.7 | | 4 | 4275.4 | baseline |
| pcbench-klangorium_logic_noise_playground | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 30.7 | +0.0 | 4 | 4275.4 | loss (cpu_s) |
| pcbench-komputer-klavier_KomputerKlavier | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 6.2 | | 0 | 745.3 | baseline |
| pcbench-komputer-klavier_KomputerKlavier | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 6.3 | +0.1 | 0 | 745.3 | loss (cpu_s) |
| pcbench-led-wordclock_wordclock | kicad | main | 0.00 | 0 | 4 | 990.0 | | unmeasured | 38.9 | | 26 | 1718.8 | baseline |
| pcbench-led-wordclock_wordclock | kicad | neckdown | 0.00 | 0 | 4 | 990.0 | 0.0 | | 39.4 | +0.5 | 26 | 1718.8 | loss (cpu_s) |
| pcbench-led_array_atmega8_led_array | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 5.2 | | 0 | 1515.7 | baseline |
| pcbench-led_array_atmega8_led_array | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 5.3 | +0.1 | 0 | 1515.7 | loss (cpu_s) |
| pcbench-lfi-rig_lfi-driver | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 5.0 | | 3 | 1237.2 | baseline |
| pcbench-lfi-rig_lfi-driver | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 5.0 | +0.0 | 3 | 1237.2 | loss (cpu_s) |
| pcbench-light-painting-wand_light-wand | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 12.1 | | 14 | 525.3 | baseline |
| pcbench-light-painting-wand_light-wand | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 12.0 | -0.1 | 14 | 525.3 | win (cpu_s) |
| pcbench-linklayer_contact | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 10.7 | | 7 | 722.2 | baseline |
| pcbench-linklayer_contact | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 10.9 | +0.2 | 7 | 722.2 | loss (cpu_s) |
| pcbench-low-power-counter_lpcounter | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.5 | | 1 | 597.4 | baseline |
| pcbench-low-power-counter_lpcounter | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.5 | +0.1 | 1 | 597.4 | loss (cpu_s) |
| pcbench-m2-electronics_m2fc | kicad | main | 0.00 | 5 | 9 | 967.8 | | unmeasured | 298.5 | | 100 | 3662.8 | baseline |
| pcbench-m2-electronics_m2fc | kicad | neckdown | 0.00 | 1 | 11 | 984.8 | +17.1 | | 300.1 | +1.6 | 104 | 3820.6 | win (unrouted) |
| pcbench-m2-electronics_m2pogo | kicad | main | 0.00 | 0 | 4 | 800.0 | | unmeasured | 0.0 | | 0 | 52.4 | baseline |
| pcbench-m2-electronics_m2pogo | kicad | neckdown | 0.00 | 0 | 4 | 800.0 | 0.0 | | 0.0 | 0.0 | 0 | 52.4 | loss (peak_rss_mb) |
| pcbench-m2-electronics_m2r | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 34.1 | | 27 | 1353.7 | baseline |
| pcbench-m2-electronics_m2r | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 34.6 | +0.5 | 27 | 1353.7 | loss (cpu_s) |
| pcbench-m2-electronics_m2rl | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.8 | | 5 | 325.3 | baseline |
| pcbench-m2-electronics_m2rl | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.9 | +0.0 | 5 | 325.3 | loss (cpu_s) |
| pcbench-mac-pro-conversion_front-panel-power-adapter | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.1 | | 1 | 204.6 | baseline |
| pcbench-mac-pro-conversion_front-panel-power-adapter | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.1 | 0.0 | 1 | 204.6 | tie (peak_rss_mb) |
| pcbench-magic-table_etch-a-sketch_cyclone | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.7 | | 0 | 826.8 | baseline |
| pcbench-magic-table_etch-a-sketch_cyclone | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.8 | +0.1 | 0 | 826.8 | loss (cpu_s) |
| pcbench-makerspace-emonth_resistor_board | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.4 | | 0 | 904.3 | baseline |
| pcbench-makerspace-emonth_resistor_board | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.4 | +0.0 | 0 | 904.3 | loss (cpu_s) |
| pcbench-marlin-neopixel-bridge_ATtiny85_Marneo | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.1 | | 0 | 135.7 | baseline |
| pcbench-marlin-neopixel-bridge_ATtiny85_Marneo | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.1 | -0.0 | 0 | 135.7 | win (cpu_s) |
| pcbench-mavbridge_mavbridge | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 20.8 | | 23 | 495.0 | baseline |
| pcbench-mavbridge_mavbridge | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 20.6 | -0.2 | 23 | 495.0 | win (cpu_s) |
| pcbench-maytal_Maytal | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.4 | | 1 | 540.0 | baseline |
| pcbench-maytal_Maytal | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.5 | +0.0 | 1 | 540.0 | loss (cpu_s) |
| pcbench-mdbwerk_mdbwerk | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 45.6 | | 20 | 568.0 | baseline |
| pcbench-mdbwerk_mdbwerk | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 45.5 | -0.2 | 20 | 568.0 | win (cpu_s) |
| pcbench-mearm-base-pcb_ServoPCB | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.2 | | 0 | 405.1 | baseline |
| pcbench-mearm-base-pcb_ServoPCB | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.2 | 0.0 | 0 | 405.1 | loss (peak_rss_mb) |
| pcbench-mechkeys_lfk78-jtag | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.1 | | 0 | 121.7 | baseline |
| pcbench-mechkeys_lfk78-jtag | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.1 | +0.0 | 0 | 121.7 | loss (cpu_s) |
| pcbench-medusa_medusa_rs422_rx | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 6.7 | | 6 | 650.3 | baseline |
| pcbench-medusa_medusa_rs422_rx | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 6.9 | +0.1 | 6 | 650.3 | loss (cpu_s) |
| pcbench-memory-display_memory-display | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.6 | | 1 | 515.5 | baseline |
| pcbench-memory-display_memory-display | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.6 | -0.0 | 1 | 515.5 | win (cpu_s) |
| pcbench-memsarray_mems_array | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 140.7 | | 64 | 9111.9 | baseline |
| pcbench-memsarray_mems_array | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 125.6 | -15.0 | 64 | 9111.9 | win (cpu_s) |
| pcbench-microphone_preamp | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 9.4 | | 3 | 333.6 | baseline |
| pcbench-microphone_preamp | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 9.6 | +0.2 | 3 | 333.6 | loss (cpu_s) |
| pcbench-mightyduino_mightyduino | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 104.8 | | 42 | 993.0 | baseline |
| pcbench-mightyduino_mightyduino | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 91.6 | -13.2 | 42 | 993.0 | win (cpu_s) |
| pcbench-mikoto_mikoto-flashbed | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.6 | | 0 | 216.5 | baseline |
| pcbench-mikoto_mikoto-flashbed | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.6 | 0.0 | 0 | 216.5 | loss (peak_rss_mb) |
| pcbench-mini_ice40_mini_ice40 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 113.0 | | 17 | 958.0 | baseline |
| pcbench-mini_ice40_mini_ice40 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 95.4 | -17.6 | 17 | 958.0 | win (cpu_s) |
| pcbench-miniboard-opamp_miniboard-opamp | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 4.1 | | 4 | 290.5 | baseline |
| pcbench-miniboard-opamp_miniboard-opamp | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 4.2 | +0.0 | 4 | 290.5 | loss (cpu_s) |
| pcbench-miniboard-stm32f0_miniboard-stm32f0 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 12.8 | | 19 | 532.1 | baseline |
| pcbench-miniboard-stm32f0_miniboard-stm32f0 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 12.8 | +0.1 | 19 | 532.1 | loss (cpu_s) |
| pcbench-mobile-sensor-pcb_mobile-sensor-pcb | kicad | main | 0.00 | 0 | 2 | 997.6 | | unmeasured | 148.0 | | 46 | 3498.4 | baseline |
| pcbench-mobile-sensor-pcb_mobile-sensor-pcb | kicad | neckdown | 0.00 | 0 | 2 | 997.6 | 0.0 | | 119.9 | -28.0 | 46 | 3498.4 | win (cpu_s) |
| pcbench-mojo-nes_mojo-nes | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 34.3 | | 25 | 1769.9 | baseline |
| pcbench-mojo-nes_mojo-nes | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 34.5 | +0.2 | 25 | 1769.9 | loss (cpu_s) |
| pcbench-motor-3xdrv8833-hw_ver1 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 165.5 | | 69 | 2217.8 | baseline |
| pcbench-motor-3xdrv8833-hw_ver1 | kicad | neckdown | 0.00 | 0 | 1 | 997.2 | -2.7 | | 147.0 | -18.5 | 65 | 2230.6 | loss (clean_pass_rate) |
| pcbench-mppt-2420-hc_mppt-2420-hc | kicad | main | 0.00 | 1 | 0 | 989.9 | | unmeasured | 139.6 | | 67 | 3470.7 | baseline |
| pcbench-mppt-2420-hc_mppt-2420-hc | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | +10.1 | | 152.9 | +13.3 | 65 | 3529.1 | win (clean_pass_rate) |
| pcbench-mppt-2420-hpx_mppt-2420-hpx | kicad | main | 0.00 | 6 | 0 | 961.5 | | unmeasured | 134.7 | | 109 | 5104.7 | baseline |
| pcbench-mppt-2420-hpx_mppt-2420-hpx | kicad | neckdown | 0.00 | 1 | 0 | 993.6 | +32.1 | | 234.0 | +99.3 | 113 | 5074.6 | win (unrouted) |
| pcbench-nRF24breakoutBoard_nRF24-breakout | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.2 | | 0 | 98.5 | baseline |
| pcbench-nRF24breakoutBoard_nRF24-breakout | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.2 | -0.0 | 0 | 98.5 | win (cpu_s) |
| pcbench-nand_programmer_adapter_tsop48 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 7.6 | | 4 | 889.6 | baseline |
| pcbench-nand_programmer_adapter_tsop48 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 7.6 | +0.1 | 4 | 889.6 | loss (cpu_s) |
| pcbench-nanoSwinSidC_nanoSwinSidC | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 70.5 | | 27 | 536.5 | baseline |
| pcbench-nanoSwinSidC_nanoSwinSidC | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 70.8 | +0.3 | 27 | 536.5 | loss (cpu_s) |
| pcbench-navelino-leaf_navelino-leaf | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 20.0 | | 13 | 348.8 | baseline |
| pcbench-navelino-leaf_navelino-leaf | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 20.3 | +0.3 | 13 | 348.8 | loss (cpu_s) |
| pcbench-nextbusclock_NextBusClockV1 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 9.8 | | 2 | 1547.0 | baseline |
| pcbench-nextbusclock_NextBusClockV1 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 9.8 | -0.0 | 2 | 1547.0 | win (cpu_s) |
| pcbench-nfl-led-scoreboard_passive-rpi-hub75-adapter | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 3.1 | | 0 | 296.7 | baseline |
| pcbench-nfl-led-scoreboard_passive-rpi-hub75-adapter | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 3.2 | +0.0 | 0 | 296.7 | loss (cpu_s) |
| pcbench-nfl-led-scoreboard_passive3-rpi-hub75-adapter | kicad | main | 0.00 | 11 | 0 | 592.6 | | unmeasured | 60.0 | | 13 | 1280.6 | baseline |
| pcbench-nfl-led-scoreboard_passive3-rpi-hub75-adapter | kicad | neckdown | 0.00 | 11 | 0 | 592.6 | 0.0 | | 55.3 | -4.7 | 13 | 1280.6 | win (cpu_s) |
| pcbench-nikon_gps_nikon_gps | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 12.3 | | 13 | 331.4 | baseline |
| pcbench-nikon_gps_nikon_gps | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 12.3 | +0.0 | 13 | 331.4 | loss (cpu_s) |
| pcbench-nixie-clock_ab18x5-breakout | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.8 | | 0 | 121.3 | baseline |
| pcbench-nixie-clock_ab18x5-breakout | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.8 | -0.0 | 0 | 121.3 | win (cpu_s) |
| pcbench-nodemcu-backstage_NodeMCU Backstage | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.4 | | 1 | 393.9 | baseline |
| pcbench-nodemcu-backstage_NodeMCU Backstage | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.5 | +0.0 | 1 | 393.9 | loss (cpu_s) |
| pcbench-nodemcu-basecamp_NodeMCU Basecamp | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 38.9 | | 20 | 1345.4 | baseline |
| pcbench-nodemcu-basecamp_NodeMCU Basecamp | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 38.8 | -0.0 | 20 | 1345.4 | win (cpu_s) |
| pcbench-nonSNES_SNSP-CPU-1CHIP | kicad | main | 0.00 | 173 | 2 | 292.2 | | unmeasured | 299.4 | | 288 | 6403.9 | baseline |
| pcbench-nonSNES_SNSP-CPU-1CHIP | kicad | neckdown | 0.00 | 122 | 1 | 501.2 | +209.0 | | 301.4 | +2.1 | 329 | 9188.7 | win (unrouted) |
| pcbench-nrf2rfm69_nrf2rfm69 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.8 | | 4 | 179.5 | baseline |
| pcbench-nrf2rfm69_nrf2rfm69 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.9 | +0.0 | 4 | 179.5 | loss (cpu_s) |
| pcbench-nunchuk_rf_hw_NunchukRF_V3 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 102.8 | | 36 | 1126.7 | baseline |
| pcbench-nunchuk_rf_hw_NunchukRF_V3 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 86.8 | -16.0 | 36 | 1126.7 | win (cpu_s) |
| pcbench-oasis_ledboard | kicad | main | 0.00 | 11 | 0 | 0.0 | | unmeasured | 10.4 | | 0 | 0.0 | baseline |
| pcbench-oasis_ledboard | kicad | neckdown | 0.00 | 5 | 9 | 381.8 | +381.8 | | 29.5 | +19.1 | 9 | 216.5 | win (unrouted) |
| pcbench-oled-bmp280-touch_oled-bmp280-touch | kicad | main | 0.00 | 0 | 2 | 987.9 | | unmeasured | 0.5 | | 1 | 531.7 | baseline |
| pcbench-oled-bmp280-touch_oled-bmp280-touch | kicad | neckdown | 0.00 | 0 | 2 | 987.9 | 0.0 | | 0.5 | +0.0 | 1 | 531.7 | loss (cpu_s) |
| pcbench-one-shift-register_one-shift-register | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.2 | | 0 | 385.9 | baseline |
| pcbench-one-shift-register_one-shift-register | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.2 | +0.1 | 0 | 385.9 | loss (cpu_s) |
| pcbench-onion2-breakout_onion2 breakout | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 6.8 | | 5 | 623.2 | baseline |
| pcbench-onion2-breakout_onion2 breakout | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 6.9 | +0.1 | 5 | 623.2 | loss (cpu_s) |
| pcbench-opentilt_opentilt2 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.4 | | 0 | 413.1 | baseline |
| pcbench-opentilt_opentilt2 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.5 | +0.1 | 0 | 413.1 | loss (cpu_s) |
| pcbench-oshtimer_transponder | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.1 | | 1 | 64.7 | baseline |
| pcbench-oshtimer_transponder | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.1 | -0.0 | 1 | 64.7 | win (cpu_s) |
| pcbench-ottawa-badges-2016_ottawa-badge-tagger-2016 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 8.2 | | 3 | 348.1 | baseline |
| pcbench-ottawa-badges-2016_ottawa-badge-tagger-2016 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 8.2 | -0.1 | 3 | 348.1 | win (cpu_s) |
| pcbench-ozinverter_ozinverterkicad | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 17.5 | | 0 | 2123.5 | baseline |
| pcbench-ozinverter_ozinverterkicad | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 17.5 | 0.0 | 0 | 2123.5 | tie (peak_rss_mb) |
| pcbench-pcb-covox-amp-v2_pcb-covox-amp-v2 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 12.8 | | 28 | 1509.8 | baseline |
| pcbench-pcb-covox-amp-v2_pcb-covox-amp-v2 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 12.9 | +0.1 | 28 | 1509.8 | loss (cpu_s) |
| pcbench-pcb-covox-amp_pcb-covox-amp | kicad | main | 0.00 | 1 | 0 | 968.7 | | unmeasured | 91.2 | | 14 | 835.9 | baseline |
| pcbench-pcb-covox-amp_pcb-covox-amp | kicad | neckdown | 0.00 | 1 | 0 | 968.7 | 0.0 | | 90.6 | -0.6 | 14 | 835.9 | win (cpu_s) |
| pcbench-pcb-ks0108-128x64-glcd_circuit | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 15.6 | | 14 | 395.1 | baseline |
| pcbench-pcb-ks0108-128x64-glcd_circuit | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 15.5 | -0.0 | 14 | 395.1 | win (cpu_s) |
| pcbench-pcb-usb-ft245r-parallel-adapter_pcb-usb-ft245r-parallel-adapter | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 3.6 | | 10 | 415.4 | baseline |
| pcbench-pcb-usb-ft245r-parallel-adapter_pcb-usb-ft245r-parallel-adapter | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 3.7 | +0.0 | 10 | 415.4 | loss (cpu_s) |
| pcbench-pesho_pesho | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 8.4 | | 2 | 1580.5 | baseline |
| pcbench-pesho_pesho | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 8.6 | +0.2 | 2 | 1580.5 | loss (cpu_s) |
| pcbench-phone_rtty_interface_phone_rtty_rev_a | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.9 | | 0 | 434.9 | baseline |
| pcbench-phone_rtty_interface_phone_rtty_rev_a | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.9 | +0.0 | 0 | 434.9 | loss (cpu_s) |
| pcbench-phone_rtty_interface_phone_rtty_rev_b | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.0 | | 0 | 507.2 | baseline |
| pcbench-phone_rtty_interface_phone_rtty_rev_b | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.0 | -0.0 | 0 | 507.2 | win (cpu_s) |
| pcbench-photon_Sprinkler_sprinkler | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.6 | | 1 | 742.4 | baseline |
| pcbench-photon_Sprinkler_sprinkler | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.7 | +0.1 | 1 | 742.4 | loss (cpu_s) |
| pcbench-pi-zero-stepper-board_pi-zero-stepper-board | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 9.3 | | 3 | 1636.9 | baseline |
| pcbench-pi-zero-stepper-board_pi-zero-stepper-board | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 9.5 | +0.2 | 3 | 1636.9 | loss (cpu_s) |
| pcbench-pi_plant_MCP3002 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.2 | | 0 | 227.7 | baseline |
| pcbench-pi_plant_MCP3002 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.2 | 0.0 | 0 | 227.7 | win (peak_rss_mb) |
| pcbench-pico-pi-rel_pico-pi | kicad | main | 0.00 | 4 | 0 | 967.2 | | unmeasured | 130.9 | | 56 | 1168.4 | baseline |
| pcbench-pico-pi-rel_pico-pi | kicad | neckdown | 0.00 | 4 | 0 | 967.2 | 0.0 | | 118.1 | -12.8 | 56 | 1168.4 | win (cpu_s) |
| pcbench-pmw3360-pcb_pmw3360_pcb_jst | kicad | main | 0.00 | 5 | 6 | 655.6 | | unmeasured | 5.4 | | 4 | 212.1 | baseline |
| pcbench-pmw3360-pcb_pmw3360_pcb_jst | kicad | neckdown | 0.00 | 5 | 6 | 655.6 | 0.0 | | 5.3 | -0.1 | 4 | 212.1 | win (cpu_s) |
| pcbench-pocketbone-kicad_pocketbone-kicad | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 211.7 | | 75 | 1596.0 | baseline |
| pcbench-pocketbone-kicad_pocketbone-kicad | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 165.5 | -46.1 | 75 | 1596.0 | win (cpu_s) |
| pcbench-polypoint_pinpoint_timebase | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 70.0 | | 38 | 1404.9 | baseline |
| pcbench-polypoint_pinpoint_timebase | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 63.9 | -6.1 | 38 | 1404.9 | win (cpu_s) |
| pcbench-ponyser-pcb_Ponyser | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.5 | | 0 | 104.7 | baseline |
| pcbench-ponyser-pcb_Ponyser | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.5 | -0.0 | 0 | 104.7 | win (cpu_s) |
| pcbench-preamp-two_input-selector | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 19.7 | | 2 | 1019.3 | baseline |
| pcbench-preamp-two_input-selector | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 20.0 | +0.3 | 2 | 1019.3 | loss (cpu_s) |
| pcbench-preamp-two_mcu-board | kicad | main | 0.00 | 1 | 0 | 977.3 | | unmeasured | 2.9 | | 3 | 392.7 | baseline |
| pcbench-preamp-two_mcu-board | kicad | neckdown | 0.00 | 1 | 0 | 977.3 | 0.0 | | 3.0 | +0.1 | 3 | 392.7 | loss (cpu_s) |
| pcbench-preamp-two_mdac-attenuator | kicad | main | 0.00 | 1 | 0 | 952.4 | | unmeasured | 3.9 | | 2 | 293.9 | baseline |
| pcbench-preamp-two_mdac-attenuator | kicad | neckdown | 0.00 | 1 | 0 | 952.4 | 0.0 | | 3.9 | -0.0 | 2 | 293.9 | win (cpu_s) |
| pcbench-prog-cc-100mA_prog-cc-100mA | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 5.9 | | 0 | 618.5 | baseline |
| pcbench-prog-cc-100mA_prog-cc-100mA | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 6.0 | +0.0 | 0 | 618.5 | loss (cpu_s) |
| pcbench-project-hydra-meshtastic-pcb_meshtastic-diy | kicad | main | 0.00 | 24 | 5 | 528.3 | | unmeasured | 128.3 | | 61 | 988.7 | baseline |
| pcbench-project-hydra-meshtastic-pcb_meshtastic-diy | kicad | neckdown | 0.00 | 24 | 5 | 528.3 | 0.0 | | 103.5 | -24.8 | 61 | 988.7 | win (cpu_s) |
| pcbench-pulse_v1_pulse | kicad | main | 0.00 | 1 | 0 | 923.1 | | unmeasured | 18.8 | | 6 | 163.0 | baseline |
| pcbench-pulse_v1_pulse | kicad | neckdown | 0.00 | 1 | 0 | 923.1 | 0.0 | | 19.0 | +0.2 | 6 | 163.0 | loss (cpu_s) |
| pcbench-pusheenz40_sadcatz40 | kicad | main | 0.00 | 43 | 0 | 516.8 | | unmeasured | 298.5 | | 78 | 1820.6 | baseline |
| pcbench-pusheenz40_sadcatz40 | kicad | neckdown | 0.00 | 43 | 0 | 516.8 | -0.0 | | 299.9 | +1.4 | 79 | 1823.4 | loss (score) |
| pcbench-pwm-2420-lus_pwm-2420-lus | kicad | main | 0.00 | 10 | 0 | 885.0 | | unmeasured | 147.8 | | 80 | 3147.1 | baseline |
| pcbench-pwm-2420-lus_pwm-2420-lus | kicad | neckdown | 0.00 | 2 | 12 | 949.4 | +64.4 | | 112.5 | -35.3 | 78 | 3189.0 | win (unrouted) |
| pcbench-radio_antenna-iridium | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 11.9 | | 2 | 319.5 | baseline |
| pcbench-radio_antenna-iridium | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 12.3 | +0.4 | 2 | 319.5 | loss (cpu_s) |
| pcbench-raspberry_pi_pullup_button_pullup_shutdown_button | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.1 | | 0 | 52.5 | baseline |
| pcbench-raspberry_pi_pullup_button_pullup_shutdown_button | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.1 | 0.0 | 0 | 52.5 | tie (peak_rss_mb) |
| pcbench-raspberry_pi_pullup_button_pullup_shutdown_button(revB) | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.1 | | 0 | 52.3 | baseline |
| pcbench-raspberry_pi_pullup_button_pullup_shutdown_button(revB) | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.1 | -0.0 | 0 | 52.3 | win (cpu_s) |
| pcbench-rc2014_bank_switcher_z80_cpm_mmu | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 5.2 | | 2 | 1079.0 | baseline |
| pcbench-rc2014_bank_switcher_z80_cpm_mmu | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 5.2 | +0.0 | 2 | 1079.0 | loss (cpu_s) |
| pcbench-real-time-chess_kfchess | kicad | main | 0.00 | 78 | 70 | 591.1 | | unmeasured | 298.9 | | 245 | 16497.4 | baseline |
| pcbench-real-time-chess_kfchess | kicad | neckdown | 0.00 | 72 | 72 | 616.0 | +24.9 | | 300.5 | +1.6 | 206 | 16556.7 | win (unrouted) |
| pcbench-recalbox-gpio-board__autosave-board | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 14.2 | | 20 | 3807.9 | baseline |
| pcbench-recalbox-gpio-board__autosave-board | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 13.9 | -0.3 | 20 | 3807.9 | win (cpu_s) |
| pcbench-recalbox-gpio-board_board | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 14.1 | | 20 | 3807.9 | baseline |
| pcbench-recalbox-gpio-board_board | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 14.2 | +0.1 | 20 | 3807.9 | loss (cpu_s) |
| pcbench-retrocon_bbb-adapter | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 3.7 | | 2 | 964.5 | baseline |
| pcbench-retrocon_bbb-adapter | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 3.8 | +0.1 | 2 | 964.5 | loss (cpu_s) |
| pcbench-retrocon_driver_board | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.5 | | 1 | 847.0 | baseline |
| pcbench-retrocon_driver_board | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.4 | -0.1 | 1 | 847.0 | win (cpu_s) |
| pcbench-retroreflectors_TANGOFLOCK | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.2 | | 0 | 127.7 | baseline |
| pcbench-retroreflectors_TANGOFLOCK | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.2 | +0.0 | 0 | 127.7 | loss (cpu_s) |
| pcbench-rfcx-sentinel-pcb_Mainboard | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 74.8 | | 44 | 2142.2 | baseline |
| pcbench-rfcx-sentinel-pcb_Mainboard | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 67.4 | -7.4 | 44 | 2142.2 | win (cpu_s) |
| pcbench-rfidBoard_rfid | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 3.7 | | 1 | 302.8 | baseline |
| pcbench-rfidBoard_rfid | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 3.6 | -0.1 | 1 | 302.8 | win (cpu_s) |
| pcbench-rgb-led_rgb-led-v2 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.4 | | 2 | 997.0 | baseline |
| pcbench-rgb-led_rgb-led-v2 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.4 | +0.0 | 2 | 997.0 | loss (cpu_s) |
| pcbench-rgb-strip-controller__autosave-rgb-strip | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.6 | | 0 | 402.8 | baseline |
| pcbench-rgb-strip-controller__autosave-rgb-strip | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.6 | -0.0 | 0 | 402.8 | win (cpu_s) |
| pcbench-rgb2ypbpr_rgb2ypbpr | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 8.9 | | 2 | 1367.4 | baseline |
| pcbench-rgb2ypbpr_rgb2ypbpr | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 8.9 | -0.1 | 2 | 1367.4 | win (cpu_s) |
| pcbench-rjw57_cpu-board | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 298.8 | | 171 | 8397.1 | baseline |
| pcbench-rjw57_cpu-board | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 300.0 | +1.2 | 158 | 8376.8 | win (score) |
| pcbench-roomba-ESP12E_roomba-esp | kicad | main | 0.00 | 0 | 3 | 976.0 | | unmeasured | 3.2 | | 6 | 721.9 | baseline |
| pcbench-roomba-ESP12E_roomba-esp | kicad | neckdown | 0.00 | 0 | 3 | 976.0 | 0.0 | | 3.4 | +0.2 | 6 | 721.9 | loss (cpu_s) |
| pcbench-rotary-encoder-breakout_rotary-encoder-breakout | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.6 | | 2 | 116.8 | baseline |
| pcbench-rotary-encoder-breakout_rotary-encoder-breakout | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.6 | -0.0 | 2 | 116.8 | win (cpu_s) |
| pcbench-royer_royer | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.7 | | 2 | 413.3 | baseline |
| pcbench-royer_royer | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.6 | -0.0 | 2 | 413.3 | win (cpu_s) |
| pcbench-rp2040-dmxsun_baseboard_2slots | kicad | main | 0.00 | 0 | 10 | 958.3 | | unmeasured | 34.6 | | 1 | 2521.1 | baseline |
| pcbench-rp2040-dmxsun_baseboard_2slots | kicad | neckdown | 0.00 | 0 | 10 | 958.3 | 0.0 | | 34.8 | +0.2 | 1 | 2521.1 | loss (cpu_s) |
| pcbench-rp2040-dmxsun_baseboard_4slots | kicad | main | 0.00 | 0 | 4 | 985.4 | | unmeasured | 47.3 | | 4 | 4238.2 | baseline |
| pcbench-rp2040-dmxsun_baseboard_4slots | kicad | neckdown | 0.00 | 0 | 4 | 985.4 | 0.0 | | 48.0 | +0.7 | 4 | 4238.2 | loss (cpu_s) |
| pcbench-rs485-moist-sensor_adapter-por | kicad | main | 0.00 | 3 | 0 | 812.5 | | unmeasured | 4.5 | | 4 | 195.9 | baseline |
| pcbench-rs485-moist-sensor_adapter-por | kicad | neckdown | 0.00 | 3 | 0 | 812.5 | 0.0 | | 4.5 | +0.1 | 4 | 195.9 | loss (cpu_s) |
| pcbench-rs485-moist-sensor_interconnect | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.2 | | 0 | 73.0 | baseline |
| pcbench-rs485-moist-sensor_interconnect | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.2 | 0.0 | 0 | 73.0 | loss (peak_rss_mb) |
| pcbench-rs485-moist-sensor_rs485-moist-sensor | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 15.7 | | 19 | 232.3 | baseline |
| pcbench-rs485-moist-sensor_rs485-moist-sensor | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 15.6 | -0.1 | 19 | 232.3 | win (cpu_s) |
| pcbench-rufs__autosave-simple_kicad_schema_and_pcb_v1 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.0 | | 0 | 50.4 | baseline |
| pcbench-rufs__autosave-simple_kicad_schema_and_pcb_v1 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.0 | 0.0 | 0 | 50.4 | tie (peak_rss_mb) |
| pcbench-rufs_aprs_tracker | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 5.9 | | 3 | 888.7 | baseline |
| pcbench-rufs_aprs_tracker | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 5.9 | -0.0 | 3 | 888.7 | win (cpu_s) |
| pcbench-rufs_dra818v_breakout_board | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.1 | | 3 | 328.9 | baseline |
| pcbench-rufs_dra818v_breakout_board | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.1 | -0.0 | 3 | 328.9 | win (cpu_s) |
| pcbench-rufs_simple_kicad_schema_and_pcb_v1 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.0 | | 0 | 50.4 | baseline |
| pcbench-rufs_simple_kicad_schema_and_pcb_v1 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.0 | 0.0 | 0 | 50.4 | tie (peak_rss_mb) |
| pcbench-rufs_smart_psu | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 3.4 | | 3 | 426.8 | baseline |
| pcbench-rufs_smart_psu | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 3.3 | -0.1 | 3 | 426.8 | win (cpu_s) |
| pcbench-rufs_spv1040_power_controller | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.3 | | 1 | 258.3 | baseline |
| pcbench-rufs_spv1040_power_controller | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.4 | +0.0 | 1 | 258.3 | loss (cpu_s) |
| pcbench-rxadc_14_rxadc_14 | kicad | main | 0.00 | 1 | 0 | 980.4 | | unmeasured | 105.0 | | 26 | 693.8 | baseline |
| pcbench-rxadc_14_rxadc_14 | kicad | neckdown | 0.00 | 1 | 0 | 980.4 | 0.0 | | 79.7 | -25.2 | 26 | 693.8 | win (cpu_s) |
| pcbench-saiboard_3x8 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 136.3 | | 66 | 5407.7 | baseline |
| pcbench-saiboard_3x8 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 115.0 | -21.3 | 66 | 5407.7 | win (cpu_s) |
| pcbench-saiboard_8x3 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 214.0 | | 114 | 5908.8 | baseline |
| pcbench-saiboard_8x3 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 163.1 | -50.9 | 114 | 5908.8 | win (cpu_s) |
| pcbench-scimpy_amp | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 9.9 | | 3 | 1485.0 | baseline |
| pcbench-scimpy_amp | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 10.1 | +0.2 | 3 | 1485.0 | loss (cpu_s) |
| pcbench-scimpy_crossover | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 5.6 | | 0 | 549.9 | baseline |
| pcbench-scimpy_crossover | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 5.6 | +0.0 | 0 | 549.9 | loss (cpu_s) |
| pcbench-scimpy_powersupply | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.6 | | 0 | 231.8 | baseline |
| pcbench-scimpy_powersupply | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.7 | +0.0 | 0 | 231.8 | loss (cpu_s) |
| pcbench-scimpy_volumebuffer | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 6.2 | | 0 | 678.3 | baseline |
| pcbench-scimpy_volumebuffer | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 6.2 | +0.0 | 0 | 678.3 | loss (cpu_s) |
| pcbench-sensorboard_DiffIR | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.8 | | 3 | 247.9 | baseline |
| pcbench-sensorboard_DiffIR | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.8 | +0.0 | 3 | 247.9 | loss (cpu_s) |
| pcbench-sensorboard_DiffIR.kicad_pcb_narrow | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.4 | | 1 | 271.6 | baseline |
| pcbench-sensorboard_DiffIR.kicad_pcb_narrow | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.4 | +0.0 | 1 | 271.6 | loss (cpu_s) |
| pcbench-shutter_Shutter V4 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 4.5 | | 1 | 711.1 | baseline |
| pcbench-shutter_Shutter V4 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 4.4 | -0.1 | 1 | 711.1 | win (cpu_s) |
| pcbench-shutter_speed_tester_shutter_speed_tester | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.5 | | 0 | 270.0 | baseline |
| pcbench-shutter_speed_tester_shutter_speed_tester | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.5 | +0.0 | 0 | 270.0 | loss (cpu_s) |
| pcbench-simplebus2-intercom_repeater_v2 | kicad | main | 0.00 | 1 | 0 | 944.4 | | unmeasured | 1.3 | | 0 | 285.8 | baseline |
| pcbench-simplebus2-intercom_repeater_v2 | kicad | neckdown | 0.00 | 1 | 0 | 944.4 | 0.0 | | 1.4 | +0.1 | 0 | 285.8 | loss (cpu_s) |
| pcbench-sms-cart-32k_cart | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 5.0 | | 16 | 975.8 | baseline |
| pcbench-sms-cart-32k_cart | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 5.0 | 0.0 | 16 | 975.8 | win (peak_rss_mb) |
| pcbench-smt-zvs-driver_IH10-sl | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.7 | | 1 | 228.2 | baseline |
| pcbench-smt-zvs-driver_IH10-sl | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.8 | +0.1 | 1 | 228.2 | loss (cpu_s) |
| pcbench-snappi-zero_snappi-zero | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 3.9 | | 3 | 454.1 | baseline |
| pcbench-snappi-zero_snappi-zero | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 3.9 | -0.1 | 3 | 454.1 | win (cpu_s) |
| pcbench-soil-moisture-sensor-analog_analog-moist-sensor | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 3.3 | | 1 | 327.7 | baseline |
| pcbench-soil-moisture-sensor-analog_analog-moist-sensor | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 3.2 | -0.1 | 1 | 327.7 | win (cpu_s) |
| pcbench-solar-lanterns_proto1 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.5 | | 1 | 176.0 | baseline |
| pcbench-solar-lanterns_proto1 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.5 | 0.0 | 1 | 176.0 | tie (peak_rss_mb) |
| pcbench-sonic3_feram_adapter_sonic3_feram_adapter | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 6.5 | | 6 | 260.3 | baseline |
| pcbench-sonic3_feram_adapter_sonic3_feram_adapter | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 6.4 | -0.1 | 6 | 260.3 | win (cpu_s) |
| pcbench-spisolator_spisolator | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 3.5 | | 6 | 276.4 | baseline |
| pcbench-spisolator_spisolator | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 3.5 | +0.0 | 6 | 276.4 | loss (cpu_s) |
| pcbench-split-pcb-throughole_splanck throughhole | kicad | main | 0.00 | 0 | 2 | 991.7 | | unmeasured | 2.3 | | 0 | 1441.2 | baseline |
| pcbench-split-pcb-throughole_splanck throughhole | kicad | neckdown | 0.00 | 0 | 2 | 991.7 | 0.0 | | 2.3 | +0.0 | 0 | 1441.2 | loss (cpu_s) |
| pcbench-srambo_1_srambo_1 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 123.1 | | 57 | 3510.9 | baseline |
| pcbench-srambo_1_srambo_1 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 95.3 | -27.8 | 57 | 3510.9 | win (cpu_s) |
| pcbench-ssr-wifi_adapter | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 7.4 | | 2 | 912.9 | baseline |
| pcbench-ssr-wifi_adapter | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 7.5 | +0.0 | 2 | 912.9 | loss (cpu_s) |
| pcbench-starfish_starfish | kicad | main | 0.00 | 1 | 0 | 978.7 | | unmeasured | 100.5 | | 46 | 813.1 | baseline |
| pcbench-starfish_starfish | kicad | neckdown | 0.00 | 1 | 0 | 978.7 | 0.0 | | 82.0 | -18.5 | 46 | 813.1 | win (cpu_s) |
| pcbench-starsynctrackers_reset_switch | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.2 | | 0 | 78.4 | baseline |
| pcbench-starsynctrackers_reset_switch | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.2 | 0.0 | 0 | 78.4 | win (peak_rss_mb) |
| pcbench-stlinkv2_breakout_stlink_breakout | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.1 | | 0 | 74.2 | baseline |
| pcbench-stlinkv2_breakout_stlink_breakout | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.1 | 0.0 | 0 | 74.2 | tie (peak_rss_mb) |
| pcbench-stm32_ccd_camera_ccd | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 68.9 | | 26 | 673.5 | baseline |
| pcbench-stm32_ccd_camera_ccd | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 56.6 | -12.4 | 26 | 673.5 | win (cpu_s) |
| pcbench-stubby_hex | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 109.1 | | 39 | 1532.4 | baseline |
| pcbench-stubby_hex | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 87.2 | -21.9 | 39 | 1532.4 | win (cpu_s) |
| pcbench-sv650sds_sds_tool | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.9 | | 1 | 302.2 | baseline |
| pcbench-sv650sds_sds_tool | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.0 | +0.0 | 1 | 302.2 | loss (cpu_s) |
| pcbench-taira-keyboard_tairakb | kicad | main | 0.00 | 10 | 0 | 821.4 | | unmeasured | 268.6 | | 168 | 3266.2 | baseline |
| pcbench-taira-keyboard_tairakb | kicad | neckdown | 0.00 | 10 | 0 | 821.4 | 0.0 | | 188.2 | -80.4 | 168 | 3266.2 | win (cpu_s) |
| pcbench-tbd_tbd | kicad | main | 0.00 | 1 | 0 | 937.5 | | unmeasured | 33.9 | | 12 | 204.1 | baseline |
| pcbench-tbd_tbd | kicad | neckdown | 0.00 | 1 | 0 | 937.5 | 0.0 | | 34.5 | +0.6 | 12 | 204.1 | loss (cpu_s) |
| pcbench-tdstat_TDstatv2 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 35.8 | | 15 | 1869.9 | baseline |
| pcbench-tdstat_TDstatv2 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 34.7 | -1.0 | 15 | 1869.9 | win (cpu_s) |
| pcbench-technoshield-ui-hw_technoshield | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 38.7 | | 19 | 2784.9 | baseline |
| pcbench-technoshield-ui-hw_technoshield | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 32.4 | -6.4 | 19 | 2784.9 | win (cpu_s) |
| pcbench-teensy-touch_teensy-touch | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.7 | | 2 | 590.0 | baseline |
| pcbench-teensy-touch_teensy-touch | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.6 | -0.0 | 2 | 590.0 | win (cpu_s) |
| pcbench-teensy-weather-badge_teensyi2c | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 7.9 | | 0 | 449.0 | baseline |
| pcbench-teensy-weather-badge_teensyi2c | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 8.5 | +0.6 | 0 | 449.0 | loss (cpu_s) |
| pcbench-teensy-wifi-weather-logger_teensyi2c | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 8.6 | | 0 | 449.0 | baseline |
| pcbench-teensy-wifi-weather-logger_teensyi2c | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 8.5 | -0.1 | 0 | 449.0 | win (cpu_s) |
| pcbench-temperature-alarm_controller | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.5 | | 1 | 835.8 | baseline |
| pcbench-temperature-alarm_controller | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.5 | 0.0 | 1 | 835.8 | win (peak_rss_mb) |
| pcbench-tepmachcha_tepmachcha | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.2 | | 1 | 206.1 | baseline |
| pcbench-tepmachcha_tepmachcha | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.2 | -0.0 | 1 | 206.1 | win (cpu_s) |
| pcbench-tessel-ice40__autosave-project | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 49.8 | | 23 | 849.9 | baseline |
| pcbench-tessel-ice40__autosave-project | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 41.6 | -8.2 | 23 | 849.9 | win (cpu_s) |
| pcbench-thatmicpre_thatmicpre_v1 | kicad | main | 0.00 | 0 | 4 | 982.2 | | unmeasured | 8.0 | | 0 | 940.0 | baseline |
| pcbench-thatmicpre_thatmicpre_v1 | kicad | neckdown | 0.00 | 0 | 4 | 982.2 | 0.0 | | 7.8 | -0.2 | 0 | 940.0 | win (cpu_s) |
| pcbench-thatmicpre_thatmicpre_v2 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 9.2 | | 0 | 1430.2 | baseline |
| pcbench-thatmicpre_thatmicpre_v2 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 9.3 | +0.1 | 0 | 1430.2 | loss (cpu_s) |
| pcbench-thegrid_thegrid | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 4.8 | | 0 | 1211.7 | baseline |
| pcbench-thegrid_thegrid | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 4.9 | +0.0 | 0 | 1211.7 | loss (cpu_s) |
| pcbench-thingBot-LoRa_thingBot-LoRa_v1P0 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 5.6 | | 4 | 339.3 | baseline |
| pcbench-thingBot-LoRa_thingBot-LoRa_v1P0 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 5.5 | -0.0 | 4 | 339.3 | win (cpu_s) |
| pcbench-thingBot-LoRa_thingBot-LoRa_v1P1 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 5.5 | | 6 | 331.5 | baseline |
| pcbench-thingBot-LoRa_thingBot-LoRa_v1P1 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 5.5 | -0.0 | 6 | 331.5 | win (cpu_s) |
| pcbench-timecircuits-hardware_BTTF-TimeCircuits | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 258.1 | | 81 | 5502.4 | baseline |
| pcbench-timecircuits-hardware_BTTF-TimeCircuits | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 193.2 | -64.9 | 81 | 5502.4 | win (cpu_s) |
| pcbench-tiny-8088_Computer | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 163.8 | | 44 | 7380.3 | baseline |
| pcbench-tiny-8088_Computer | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 114.9 | -48.9 | 44 | 7380.3 | win (cpu_s) |
| pcbench-tinyFISH_tinyBRUSH | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 1.3 | | 5 | 60.5 | baseline |
| pcbench-tinyFISH_tinyBRUSH | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.3 | +0.1 | 5 | 60.5 | loss (cpu_s) |
| pcbench-tinyisp-micro_tinyispmicro | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 8.0 | | 13 | 342.6 | baseline |
| pcbench-tinyisp-micro_tinyispmicro | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 7.9 | -0.0 | 13 | 342.6 | win (cpu_s) |
| pcbench-tinymuseum_museum | kicad | main | 0.00 | 1 | 0 | 961.5 | | unmeasured | 103.6 | | 3 | 701.4 | baseline |
| pcbench-tinymuseum_museum | kicad | neckdown | 0.00 | 1 | 0 | 961.5 | 0.0 | | 87.7 | -15.9 | 3 | 701.4 | win (cpu_s) |
| pcbench-type5_type5 | kicad | main | 0.00 | 0 | 13 | 952.7 | | unmeasured | 11.5 | | 16 | 1898.6 | baseline |
| pcbench-type5_type5 | kicad | neckdown | 0.00 | 0 | 13 | 952.7 | 0.0 | | 11.9 | +0.4 | 16 | 1898.6 | loss (cpu_s) |
| pcbench-uC3Moy_uC3Moy | kicad | main | 0.00 | 0 | 9 | 871.4 | | unmeasured | 1.6 | | 0 | 338.3 | baseline |
| pcbench-uC3Moy_uC3Moy | kicad | neckdown | 0.00 | 0 | 9 | 871.4 | 0.0 | | 1.6 | -0.0 | 0 | 338.3 | win (cpu_s) |
| pcbench-uSKY_uSKY | kicad | main | 0.00 | 24 | 0 | 351.4 | | unmeasured | 48.1 | | 3 | 70.2 | baseline |
| pcbench-uSKY_uSKY | kicad | neckdown | 0.00 | 24 | 0 | 351.4 | 0.0 | | 41.7 | -6.4 | 3 | 70.2 | win (cpu_s) |
| pcbench-uext-esp32_UEXT_ESP32 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 3.0 | | 6 | 280.3 | baseline |
| pcbench-uext-esp32_UEXT_ESP32 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 3.0 | +0.0 | 6 | 280.3 | loss (cpu_s) |
| pcbench-usb_rs232c_usb_rs232c_rev_a | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 17.9 | | 16 | 339.9 | baseline |
| pcbench-usb_rs232c_usb_rs232c_rev_a | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 17.7 | -0.2 | 16 | 339.9 | win (cpu_s) |
| pcbench-vatx_vatx | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 8.7 | | 6 | 669.9 | baseline |
| pcbench-vatx_vatx | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 8.8 | +0.0 | 6 | 669.9 | loss (cpu_s) |
| pcbench-vdcmon_vdcmon | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 6.0 | | 2 | 1228.5 | baseline |
| pcbench-vdcmon_vdcmon | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 6.0 | -0.0 | 2 | 1228.5 | win (cpu_s) |
| pcbench-wavegen_rev3 | kicad | main | 0.00 | 14 | 0 | 813.3 | | unmeasured | 298.9 | | 91 | 2860.6 | baseline |
| pcbench-wavegen_rev3 | kicad | neckdown | 0.00 | 9 | 0 | 880.0 | +66.7 | | 300.3 | +1.4 | 99 | 2902.6 | win (unrouted) |
| pcbench-wavegen_waveform-generator | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 127.0 | | 28 | 1302.3 | baseline |
| pcbench-wavegen_waveform-generator | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 84.6 | -42.5 | 32 | 1249.5 | loss (score) |
| pcbench-wavegen_waveform-generator-rev1 | kicad | main | 0.00 | 2 | 0 | 957.4 | | unmeasured | 99.8 | | 46 | 1208.7 | baseline |
| pcbench-wavegen_waveform-generator-rev1 | kicad | neckdown | 0.00 | 2 | 0 | 957.4 | 0.0 | | 81.7 | -18.1 | 46 | 1208.7 | win (cpu_s) |
| pcbench-wavegen_wavegen | kicad | main | 0.00 | 2 | 0 | 957.4 | | unmeasured | 99.8 | | 46 | 1208.7 | baseline |
| pcbench-wavegen_wavegen | kicad | neckdown | 0.00 | 2 | 0 | 957.4 | 0.0 | | 71.4 | -28.4 | 46 | 1208.7 | win (cpu_s) |
| pcbench-wifiLCD_wifilcd | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 7.0 | | 16 | 640.5 | baseline |
| pcbench-wifiLCD_wifilcd | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 6.4 | -0.6 | 16 | 640.5 | win (cpu_s) |
| pcbench-xmasOrn_xmasOrn | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 219.4 | | 47 | 2139.0 | baseline |
| pcbench-xmasOrn_xmasOrn | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 162.3 | -57.1 | 47 | 2139.0 | win (cpu_s) |
| pcbench-xwhatits-capsense-controller_model-f-3178-adaptor | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.7 | | 0 | 230.6 | baseline |
| pcbench-xwhatits-capsense-controller_model-f-3178-adaptor | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.7 | 0.0 | 0 | 230.6 | tie (peak_rss_mb) |
| pcbench-z2amiller_sensorboard_programmer | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 0.3 | | 1 | 158.7 | baseline |
| pcbench-z2amiller_sensorboard_programmer | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.3 | -0.0 | 1 | 158.7 | win (cpu_s) |
| pcbench-zx-sizif-128_sizif128 | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 234.2 | | 63 | 8460.2 | baseline |
| pcbench-zx-sizif-128_sizif128 | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 177.6 | -56.7 | 63 | 8460.2 | win (cpu_s) |
| pcbench-zx-sizif-512-ext_sizif512ext | kicad | main | 0.00 | 78 | 0 | 564.2 | | unmeasured | 298.8 | | 218 | 8225.4 | baseline |
| pcbench-zx-sizif-512-ext_sizif512ext | kicad | neckdown | 0.00 | 58 | 0 | 676.0 | +111.7 | | 300.8 | +2.0 | 241 | 8941.8 | win (unrouted) |
| pcbench-zx-sizif-512-wifi_sizif512-wifi | kicad | main | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 2.5 | | 8 | 250.0 | baseline |
| pcbench-zx-sizif-512-wifi_sizif512-wifi | kicad | neckdown | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 2.5 | 0.0 | 8 | 250.0 | tie (peak_rss_mb) |
| pcbench-zx-sizif-xxs_sizif-xxs | kicad | main | 0.00 | 22 | 0 | 821.1 | | unmeasured | 298.7 | | 124 | 2678.1 | baseline |
| pcbench-zx-sizif-xxs_sizif-xxs | kicad | neckdown | 0.00 | 22 | 0 | 821.1 | 0.0 | | 236.0 | -62.7 | 124 | 2678.1 | win (cpu_s) |

Note: seeds=1 (<3), so the noise floor could not be measured (stddev needs ≥ 3 samples) and is reported as "unmeasured" above; verdicts on this report may be less reliable than one run with ≥ 3 seeds.

## Self-report disagreements

| board | candidate | seed | self unrouted | referee unrouted | self viol | referee viol |
|---|---|---|---|---|---|---|
| kicad-issue184-motorizedopener--motorizedopener | main | 1 | 61 | 61 | 0 | 54 |
| kicad-issue184-motorizedopener--motorizedopener | neckdown | 1 | 54 | 54 | 0 | 61 |
| kicad-issue269-min_fr_test--min_fr_test | main | 1 | 1 | 0 | 0 | 0 |
| kicad-issue269-min_fr_test--min_fr_test | neckdown | 1 | 1 | 0 | 0 | 0 |
| kicad-issue558-dev-board-autoroute-demo--dev-board | main | 1 | 0 | 0 | 0 | 4 |
| kicad-issue558-dev-board-autoroute-demo--dev-board | neckdown | 1 | 0 | 0 | 0 | 4 |
| pcbench-8bit-cpu_arduino_eeprom_programmer | main | 1 | 0 | 2 | 0 | 0 |
| pcbench-8bit-cpu_arduino_eeprom_programmer | neckdown | 1 | 0 | 2 | 0 | 0 |
| pcbench-8bit-cpu_programming_interface | main | 1 | 0 | 5 | 0 | 0 |
| pcbench-8bit-cpu_programming_interface | neckdown | 1 | 0 | 5 | 0 | 0 |
| pcbench-AIOsense_AIOsense | main | 1 | 17 | 18 | 0 | 0 |
| pcbench-AIOsense_AIOsense | neckdown | 1 | 17 | 18 | 0 | 0 |
| pcbench-APM-RPi-Shield_APM-RPi-Shield | main | 1 | 0 | 0 | 0 | 1 |
| pcbench-BLDC-controller_BLDC_controller | main | 1 | 12 | 12 | 0 | 9 |
| pcbench-BLDC-controller_BLDC_controller | neckdown | 1 | 11 | 11 | 0 | 13 |
| pcbench-Blitz_.C68 | main | 1 | 609 | 499 | 0 | 0 |
| pcbench-Blitz_.C68 | neckdown | 1 | 542 | 499 | 0 | 0 |
| pcbench-Blitz_Rev.K_C68 | main | 1 | 631 | 499 | 0 | 0 |
| pcbench-Blitz_Rev.K_C68 | neckdown | 1 | 520 | 499 | 0 | 0 |
| pcbench-Blitz_copy | main | 1 | 631 | 499 | 0 | 0 |
| pcbench-Blitz_copy | neckdown | 1 | 503 | 499 | 0 | 0 |
| pcbench-Box0-hv-analog-breakoutboard_breakout | main | 1 | 134 | 128 | 0 | 0 |
| pcbench-Box0-hv-analog-breakoutboard_breakout | neckdown | 1 | 134 | 128 | 0 | 0 |
| pcbench-Brushless_ESC_Brushless_ESC | main | 1 | 0 | 0 | 0 | 1 |
| pcbench-Brushless_ESC_Brushless_ESC | neckdown | 1 | 0 | 0 | 0 | 1 |
| pcbench-CATs-Eurosynth_4CH_Mixer | main | 1 | 0 | 2 | 0 | 0 |
| pcbench-CATs-Eurosynth_4CH_Mixer | neckdown | 1 | 0 | 2 | 0 | 0 |
| pcbench-CATs-Eurosynth_Abakus_Main | main | 1 | 0 | 15 | 0 | 0 |
| pcbench-CATs-Eurosynth_Abakus_Main | neckdown | 1 | 0 | 15 | 0 | 0 |
| pcbench-CATs-Eurosynth_Arduino_VCO_Main | main | 1 | 0 | 7 | 0 | 0 |
| pcbench-CATs-Eurosynth_Arduino_VCO_Main | neckdown | 1 | 0 | 7 | 0 | 0 |
| pcbench-CATs-Eurosynth_Buffered_Multiple_Main | main | 1 | 0 | 1 | 0 | 0 |
| pcbench-CATs-Eurosynth_Buffered_Multiple_Main | neckdown | 1 | 0 | 1 | 0 | 0 |
| pcbench-CATs-Eurosynth_Buffered_Multiple_SMD_Main | main | 1 | 0 | 2 | 0 | 6 |
| pcbench-CATs-Eurosynth_Buffered_Multiple_SMD_Main | neckdown | 1 | 0 | 2 | 0 | 6 |
| pcbench-CATs-Eurosynth_CGS_Analog_Switch_Matrix_Main | main | 1 | 0 | 8 | 0 | 0 |
| pcbench-CATs-Eurosynth_CGS_Analog_Switch_Matrix_Main | neckdown | 1 | 0 | 8 | 0 | 0 |
| pcbench-CATs-Eurosynth_Classic_ADSR_Main | main | 1 | 0 | 3 | 0 | 0 |
| pcbench-CATs-Eurosynth_Classic_ADSR_Main | neckdown | 1 | 0 | 3 | 0 | 0 |
| pcbench-CATs-Eurosynth_Clock_Divider_Main | main | 1 | 0 | 1 | 0 | 0 |
| pcbench-CATs-Eurosynth_Clock_Divider_Main | neckdown | 1 | 0 | 1 | 0 | 0 |
| pcbench-CATs-Eurosynth_Envelope_Follower_Main | main | 1 | 0 | 0 | 0 | 4 |
| pcbench-CATs-Eurosynth_Envelope_Follower_Main | neckdown | 1 | 0 | 0 | 0 | 4 |
| pcbench-CATs-Eurosynth_HAGIWO_6Ch_Gate_Sequencer_Main | main | 1 | 0 | 1 | 0 | 0 |
| pcbench-CATs-Eurosynth_HAGIWO_6Ch_Gate_Sequencer_Main | neckdown | 1 | 0 | 1 | 0 | 0 |
| pcbench-CATs-Eurosynth_HAGIWO_MultiOut_Main | main | 1 | 0 | 7 | 0 | 0 |
| pcbench-CATs-Eurosynth_HAGIWO_MultiOut_Main | neckdown | 1 | 0 | 7 | 0 | 0 |
| pcbench-CATs-Eurosynth_HAGIWO_Ring_Modulator_Main | main | 1 | 0 | 4 | 0 | 0 |
| pcbench-CATs-Eurosynth_HAGIWO_Ring_Modulator_Main | neckdown | 1 | 0 | 4 | 0 | 0 |
| pcbench-CATs-Eurosynth_LFO_Main | main | 1 | 0 | 3 | 0 | 0 |
| pcbench-CATs-Eurosynth_LFO_Main | neckdown | 1 | 0 | 3 | 0 | 0 |
| pcbench-CATs-Eurosynth_MFOS_Eight_Stage_Phase_Shifter_-_Main | main | 1 | 0 | 20 | 0 | 0 |
| pcbench-CATs-Eurosynth_MFOS_Eight_Stage_Phase_Shifter_-_Main | neckdown | 1 | 0 | 20 | 0 | 0 |
| pcbench-CATs-Eurosynth_Main_Rectifier_Main | main | 1 | 0 | 0 | 0 | 3 |
| pcbench-CATs-Eurosynth_Main_Rectifier_Main | neckdown | 1 | 0 | 0 | 0 | 3 |
| pcbench-CATs-Eurosynth_Manual_Gate_Main | main | 1 | 0 | 5 | 0 | 0 |
| pcbench-CATs-Eurosynth_Manual_Gate_Main | neckdown | 1 | 0 | 5 | 0 | 0 |
| pcbench-CATs-Eurosynth_Slim_Precision_Adder_Main | main | 1 | 0 | 4 | 0 | 0 |
| pcbench-CATs-Eurosynth_Slim_Precision_Adder_Main | neckdown | 1 | 0 | 4 | 0 | 0 |
| pcbench-CATs-Eurosynth_Slimline_Voltage_Controlled_Switch_-_Main | main | 1 | 0 | 5 | 0 | 0 |
| pcbench-CATs-Eurosynth_Slimline_Voltage_Controlled_Switch_-_Main | neckdown | 1 | 0 | 5 | 0 | 0 |
| pcbench-CATs-Eurosynth_YuSynth_Improved_Steiner_VCF_Main | main | 1 | 0 | 3 | 0 | 0 |
| pcbench-CATs-Eurosynth_YuSynth_Improved_Steiner_VCF_Main | neckdown | 1 | 0 | 3 | 0 | 0 |
| pcbench-DC25_DC25 | main | 1 | 0 | 0 | 0 | 11 |
| pcbench-DC25_DC25 | neckdown | 1 | 0 | 0 | 0 | 11 |
| pcbench-DPS-1200FB_Adapter_Adapter | main | 1 | 0 | 0 | 0 | 2 |
| pcbench-DPS-1200FB_Adapter_Adapter | neckdown | 1 | 0 | 0 | 0 | 2 |
| pcbench-DerKnopf_digi-pot | neckdown | 1 | 0 | 0 | 0 | 1 |
| pcbench-Doorman_doorman_slot | main | 1 | 0 | 6 | 0 | 0 |
| pcbench-Doorman_doorman_slot | neckdown | 1 | 0 | 6 | 0 | 0 |
| pcbench-ESP07-Breakout_ESP07-Breakout | main | 1 | 0 | 0 | 0 | 2 |
| pcbench-ESP07-Breakout_ESP07-Breakout | neckdown | 1 | 0 | 0 | 0 | 2 |
| pcbench-EnvOpenPico_EliteMicro2040 | main | 1 | 0 | 18 | 0 | 0 |
| pcbench-EnvOpenPico_EliteMicro2040 | neckdown | 1 | 0 | 18 | 0 | 0 |
| pcbench-EuroPi_europi-surface-mount | main | 1 | 0 | 27 | 0 | 18 |
| pcbench-EuroPi_europi-surface-mount | neckdown | 1 | 0 | 27 | 0 | 18 |
| pcbench-Feather-ICE40-PCB_feather_ice40 | main | 1 | 5 | 26 | 0 | 0 |
| pcbench-Feather-ICE40-PCB_feather_ice40 | neckdown | 1 | 5 | 26 | 0 | 0 |
| pcbench-GameTiger_GameTiger | main | 1 | 3 | 26 | 0 | 199 |
| pcbench-GameTiger_GameTiger | neckdown | 1 | 3 | 26 | 0 | 199 |
| pcbench-Hardware_Playground_serial_gw_ATMEGA328P | neckdown | 1 | 1 | 1 | 0 | 2 |
| pcbench-Hubble_12_bit_analog_out | main | 1 | 0 | 5 | 0 | 0 |
| pcbench-Hubble_12_bit_analog_out | neckdown | 1 | 0 | 5 | 0 | 0 |
| pcbench-Hubble_16_bit_analog_out | main | 1 | 0 | 7 | 0 | 0 |
| pcbench-Hubble_16_bit_analog_out | neckdown | 1 | 0 | 7 | 0 | 0 |
| pcbench-Hubble_leds | main | 1 | 0 | 4 | 0 | 0 |
| pcbench-Hubble_leds | neckdown | 1 | 0 | 4 | 0 | 0 |
| pcbench-Hubble_mux | main | 1 | 0 | 1 | 0 | 0 |
| pcbench-Hubble_mux | neckdown | 1 | 0 | 1 | 0 | 0 |
| pcbench-Kefersender_UKW TX | main | 1 | 6 | 5 | 0 | 0 |
| pcbench-Kefersender_UKW TX | neckdown | 1 | 6 | 5 | 0 | 0 |
| pcbench-LPC2148_Stick_LPC2148_stick | neckdown | 1 | 0 | 0 | 0 | 3 |
| pcbench-LPC2148_Stick__autosave-LPC2148_stick | neckdown | 1 | 0 | 0 | 0 | 3 |
| pcbench-LittleArduinoProjects_LEDx16_board | main | 1 | 0 | 7 | 0 | 0 |
| pcbench-LittleArduinoProjects_LEDx16_board | neckdown | 1 | 0 | 7 | 0 | 0 |
| pcbench-Mouse_Mouse | main | 1 | 0 | 4 | 0 | 0 |
| pcbench-Mouse_Mouse | neckdown | 1 | 0 | 4 | 0 | 0 |
| pcbench-NavigationThing_NavigationThing | main | 1 | 0 | 0 | 0 | 3 |
| pcbench-NavigationThing_NavigationThing | neckdown | 1 | 0 | 0 | 0 | 3 |
| pcbench-OpenHardwareExG_Shield_OpenHardwareExG_Shield | main | 1 | 3 | 3 | 0 | 34 |
| pcbench-OpenHardwareExG_Shield_OpenHardwareExG_Shield | neckdown | 1 | 1 | 1 | 0 | 30 |
| pcbench-OpenHardwareExG_Shield_OpenHardwareExG_Shield_Test_Board | main | 1 | 0 | 0 | 0 | 2 |
| pcbench-OpenHardwareExG_Shield_OpenHardwareExG_Shield_Test_Board | neckdown | 1 | 0 | 0 | 0 | 2 |
| pcbench-OpenHardwareExG_Shield_OpenHardwareExG_Shield_Test_Board_all_panelled | main | 1 | 0 | 0 | 0 | 2 |
| pcbench-OpenHardwareExG_Shield_OpenHardwareExG_Shield_Test_Board_all_panelled | neckdown | 1 | 0 | 0 | 0 | 2 |
| pcbench-PCB_constant_current_ac_hv | main | 1 | 0 | 0 | 0 | 8 |
| pcbench-PCB_constant_current_ac_hv | neckdown | 1 | 0 | 0 | 0 | 8 |
| pcbench-Patternflow_patternflow | main | 1 | 0 | 0 | 0 | 3 |
| pcbench-Patternflow_patternflow | neckdown | 1 | 0 | 0 | 0 | 3 |
| pcbench-Pi1541io_Pi1541io | main | 1 | 1 | 7 | 0 | 0 |
| pcbench-Pi1541io_Pi1541io | neckdown | 1 | 1 | 7 | 0 | 0 |
| pcbench-Pi5_PCIe_Pi5_PCIe | main | 1 | 5 | 14 | 0 | 10 |
| pcbench-Pi5_PCIe_Pi5_PCIe | neckdown | 1 | 5 | 14 | 0 | 10 |
| pcbench-R1002_R1002 | main | 1 | 9 | 3 | 0 | 0 |
| pcbench-R1002_R1002 | neckdown | 1 | 9 | 3 | 0 | 0 |
| pcbench-RC6502-Apple-1-Replica_RC6502_Apple_1_SBC | main | 1 | 0 | 16 | 0 | 0 |
| pcbench-RC6502-Apple-1-Replica_RC6502_Apple_1_SBC | neckdown | 1 | 0 | 16 | 0 | 0 |
| pcbench-RC6502-Apple-1-Replica_RC6502_Terminal | main | 1 | 0 | 8 | 0 | 0 |
| pcbench-RC6502-Apple-1-Replica_RC6502_Terminal | neckdown | 1 | 0 | 8 | 0 | 0 |
| pcbench-RC6502-Apple-1-Replica_RC6502_VDU | main | 1 | 0 | 18 | 0 | 0 |
| pcbench-RC6502-Apple-1-Replica_RC6502_VDU | neckdown | 1 | 0 | 18 | 0 | 0 |
| pcbench-RX5808_rx5808_4button | main | 1 | 1 | 0 | 0 | 0 |
| pcbench-RX5808_rx5808_4button | neckdown | 1 | 1 | 0 | 0 | 0 |
| pcbench-ReSDMAC_ReSDMAC | main | 1 | 93 | 124 | 0 | 0 |
| pcbench-ReSDMAC_ReSDMAC | neckdown | 1 | 83 | 113 | 0 | 0 |
| pcbench-RetroWiFiModem_RetroWiFiModem | main | 1 | 0 | 0 | 0 | 1 |
| pcbench-RetroWiFiModem_RetroWiFiModem | neckdown | 1 | 0 | 0 | 0 | 1 |
| pcbench-TK44_TK44 | main | 1 | 15 | 15 | 0 | 1 |
| pcbench-TK44_TK44 | neckdown | 1 | 15 | 15 | 0 | 1 |
| pcbench-TX5823_TX5823 | main | 1 | 4 | 3 | 0 | 2 |
| pcbench-TX5823_TX5823 | neckdown | 1 | 4 | 3 | 0 | 2 |
| pcbench-VC4000MultiROM_MultiRomCard | main | 1 | 2 | 2 | 0 | 13 |
| pcbench-VC4000MultiROM_MultiRomCard | neckdown | 1 | 2 | 2 | 0 | 13 |
| pcbench-amalthea_amalthea_rev0 | main | 1 | 9 | 23 | 0 | 0 |
| pcbench-amalthea_amalthea_rev0 | neckdown | 1 | 9 | 23 | 0 | 0 |
| pcbench-autohat-board_inverted-usd-adapter | main | 1 | 0 | 0 | 0 | 7 |
| pcbench-autohat-board_inverted-usd-adapter | neckdown | 1 | 0 | 0 | 0 | 7 |
| pcbench-avr-fuser-32_adapter | main | 1 | 0 | 0 | 0 | 24 |
| pcbench-avr-fuser-32_adapter | neckdown | 1 | 0 | 0 | 0 | 24 |
| pcbench-avr_ledprojector_avr_ledprojection-0402 | main | 1 | 1 | 1 | 0 | 4 |
| pcbench-avr_ledprojector_avr_ledprojection-0402 | neckdown | 1 | 1 | 1 | 0 | 6 |
| pcbench-azalea_azalea | main | 1 | 39 | 45 | 0 | 0 |
| pcbench-azalea_azalea | neckdown | 1 | 39 | 45 | 0 | 0 |
| pcbench-bms-8s50-ic_bms-8s50-ic | neckdown | 1 | 40 | 40 | 0 | 10 |
| pcbench-boatcontrol_CommonCathode60A | main | 1 | 95 | 93 | 0 | 0 |
| pcbench-boatcontrol_CommonCathode60A | neckdown | 1 | 95 | 93 | 0 | 0 |
| pcbench-disco-dongle_DiscoDongle | main | 1 | 0 | 0 | 0 | 1 |
| pcbench-disco-dongle_DiscoDongle | neckdown | 1 | 0 | 0 | 0 | 1 |
| pcbench-domotics_base-board-arranged | main | 1 | 0 | 0 | 2 | 3 |
| pcbench-domotics_base-board-arranged | neckdown | 1 | 0 | 0 | 2 | 3 |
| pcbench-eeg_brainboard_batteryv0 | main | 1 | 5 | 1 | 0 | 0 |
| pcbench-eeg_brainboard_batteryv0 | neckdown | 1 | 5 | 1 | 0 | 0 |
| pcbench-epaper-102_epaper-102 | main | 1 | 2 | 6 | 0 | 0 |
| pcbench-epaper-102_epaper-102 | neckdown | 1 | 2 | 6 | 0 | 0 |
| pcbench-ergo-snm-keyboard_receiver | main | 1 | 5 | 10 | 0 | 0 |
| pcbench-ergo-snm-keyboard_receiver | neckdown | 1 | 5 | 10 | 0 | 0 |
| pcbench-esp32-4-channel-relays_esp32-4-channel-relays | main | 1 | 3 | 7 | 0 | 0 |
| pcbench-esp32-4-channel-relays_esp32-4-channel-relays | neckdown | 1 | 3 | 7 | 0 | 0 |
| pcbench-espalarm_alarm | main | 1 | 0 | 0 | 0 | 73 |
| pcbench-espalarm_alarm | neckdown | 1 | 0 | 0 | 0 | 73 |
| pcbench-esper_EsperDNS | main | 1 | 0 | 9 | 0 | 0 |
| pcbench-esper_EsperDNS | neckdown | 1 | 0 | 9 | 0 | 0 |
| pcbench-esper_programmer | main | 1 | 2 | 12 | 0 | 0 |
| pcbench-esper_programmer | neckdown | 1 | 2 | 12 | 0 | 0 |
| pcbench-ftdi-jtag-programmer_JTAGProgrammer | main | 1 | 7 | 12 | 0 | 2 |
| pcbench-ftdi-jtag-programmer_JTAGProgrammer | neckdown | 1 | 2 | 5 | 0 | 2 |
| pcbench-gb-hardware_GB-BRK-M-XS | main | 1 | 0 | 0 | 0 | 2 |
| pcbench-gb-hardware_GB-BRK-M-XS | neckdown | 1 | 0 | 0 | 0 | 2 |
| pcbench-gb-hardware_GB-CART256K-A | main | 1 | 5 | 8 | 0 | 0 |
| pcbench-gb-hardware_GB-CART256K-A | neckdown | 1 | 2 | 6 | 0 | 0 |
| pcbench-gb-hardware_GB-CART32K-A | main | 1 | 0 | 1 | 0 | 0 |
| pcbench-gb-hardware_GB-CART32K-A | neckdown | 1 | 0 | 1 | 0 | 0 |
| pcbench-gb-hardware_GB-CARTPP-XC | main | 1 | 0 | 9 | 0 | 0 |
| pcbench-gb-hardware_GB-CARTPP-XC | neckdown | 1 | 0 | 9 | 0 | 0 |
| pcbench-gb-hardware_GB-LIVE32 | main | 1 | 0 | 5 | 0 | 0 |
| pcbench-gb-hardware_GB-LIVE32 | neckdown | 1 | 0 | 5 | 0 | 0 |
| pcbench-gb-hardware_GB-MBCTEST | main | 1 | 0 | 3 | 0 | 0 |
| pcbench-gb-hardware_GB-MBCTEST | neckdown | 1 | 0 | 3 | 0 | 0 |
| pcbench-hbr-mk2_hbr-mk2-bpfs | main | 1 | 0 | 7 | 0 | 0 |
| pcbench-hbr-mk2_hbr-mk2-bpfs | neckdown | 1 | 0 | 7 | 0 | 0 |
| pcbench-hbr-mk2_hbr-mk2-digital | main | 1 | 0 | 14 | 0 | 0 |
| pcbench-hbr-mk2_hbr-mk2-digital | neckdown | 1 | 0 | 14 | 0 | 0 |
| pcbench-juno-chorus-clone_juno-chorus-clone | main | 1 | 0 | 0 | 0 | 2 |
| pcbench-juno-chorus-clone_juno-chorus-clone | neckdown | 1 | 0 | 0 | 0 | 2 |
| pcbench-karabas-nano_karabas-nano-revC | main | 1 | 97 | 97 | 0 | 1 |
| pcbench-karabas-nano_karabas-nano-revC | neckdown | 1 | 68 | 68 | 0 | 1 |
| pcbench-keyboards_Djinn | main | 1 | 933 | 386 | 0 | 0 |
| pcbench-keyboards_Djinn | neckdown | 1 | 933 | 386 | 0 | 0 |
| pcbench-kinetoscope_microcontroller | main | 1 | 1 | 49 | 0 | 0 |
| pcbench-kinetoscope_microcontroller | neckdown | 1 | 1 | 49 | 0 | 0 |
| pcbench-kinetoscope_sram-bank | main | 1 | 30 | 103 | 0 | 0 |
| pcbench-kinetoscope_sram-bank | neckdown | 1 | 32 | 105 | 0 | 0 |
| pcbench-kitspace_DIY_detector | main | 1 | 0 | 0 | 0 | 1 |
| pcbench-kitspace_DIY_detector | neckdown | 1 | 0 | 0 | 0 | 1 |
| pcbench-kitspace_OSO-BOOK-C1 | main | 1 | 56 | 7 | 0 | 32 |
| pcbench-kitspace_OSO-BOOK-C1 | neckdown | 1 | 10 | 5 | 0 | 32 |
| pcbench-kitspace_pmt_combiner | main | 1 | 0 | 0 | 0 | 1 |
| pcbench-kitspace_pmt_combiner | neckdown | 1 | 0 | 0 | 0 | 1 |
| pcbench-led-wordclock_wordclock | main | 1 | 0 | 0 | 0 | 4 |
| pcbench-led-wordclock_wordclock | neckdown | 1 | 0 | 0 | 0 | 4 |
| pcbench-m2-electronics_m2fc | main | 1 | 5 | 5 | 0 | 9 |
| pcbench-m2-electronics_m2fc | neckdown | 1 | 1 | 1 | 0 | 11 |
| pcbench-m2-electronics_m2pogo | main | 1 | 0 | 0 | 0 | 4 |
| pcbench-m2-electronics_m2pogo | neckdown | 1 | 0 | 0 | 0 | 4 |
| pcbench-mobile-sensor-pcb_mobile-sensor-pcb | main | 1 | 0 | 0 | 0 | 2 |
| pcbench-mobile-sensor-pcb_mobile-sensor-pcb | neckdown | 1 | 0 | 0 | 0 | 2 |
| pcbench-motor-3xdrv8833-hw_ver1 | neckdown | 1 | 0 | 0 | 0 | 1 |
| pcbench-nfl-led-scoreboard_passive3-rpi-hub75-adapter | main | 1 | 2 | 11 | 0 | 0 |
| pcbench-nfl-led-scoreboard_passive3-rpi-hub75-adapter | neckdown | 1 | 2 | 11 | 0 | 0 |
| pcbench-nonSNES_SNSP-CPU-1CHIP | main | 1 | 104 | 173 | 0 | 2 |
| pcbench-nonSNES_SNSP-CPU-1CHIP | neckdown | 1 | 37 | 122 | 0 | 1 |
| pcbench-oasis_ledboard | main | 1 | 6 | 11 | 0 | 0 |
| pcbench-oasis_ledboard | neckdown | 1 | 2 | 5 | 0 | 9 |
| pcbench-oled-bmp280-touch_oled-bmp280-touch | main | 1 | 0 | 0 | 0 | 2 |
| pcbench-oled-bmp280-touch_oled-bmp280-touch | neckdown | 1 | 0 | 0 | 0 | 2 |
| pcbench-pmw3360-pcb_pmw3360_pcb_jst | main | 1 | 0 | 5 | 0 | 6 |
| pcbench-pmw3360-pcb_pmw3360_pcb_jst | neckdown | 1 | 0 | 5 | 0 | 6 |
| pcbench-preamp-two_mcu-board | main | 1 | 0 | 1 | 0 | 0 |
| pcbench-preamp-two_mcu-board | neckdown | 1 | 0 | 1 | 0 | 0 |
| pcbench-project-hydra-meshtastic-pcb_meshtastic-diy | main | 1 | 6 | 24 | 0 | 5 |
| pcbench-project-hydra-meshtastic-pcb_meshtastic-diy | neckdown | 1 | 6 | 24 | 0 | 5 |
| pcbench-pusheenz40_sadcatz40 | main | 1 | 46 | 43 | 0 | 0 |
| pcbench-pusheenz40_sadcatz40 | neckdown | 1 | 46 | 43 | 0 | 0 |
| pcbench-pwm-2420-lus_pwm-2420-lus | neckdown | 1 | 2 | 2 | 0 | 12 |
| pcbench-real-time-chess_kfchess | main | 1 | 0 | 78 | 0 | 70 |
| pcbench-real-time-chess_kfchess | neckdown | 1 | 0 | 72 | 0 | 72 |
| pcbench-roomba-ESP12E_roomba-esp | main | 1 | 0 | 0 | 0 | 3 |
| pcbench-roomba-ESP12E_roomba-esp | neckdown | 1 | 0 | 0 | 0 | 3 |
| pcbench-rp2040-dmxsun_baseboard_2slots | main | 1 | 0 | 0 | 0 | 10 |
| pcbench-rp2040-dmxsun_baseboard_2slots | neckdown | 1 | 0 | 0 | 0 | 10 |
| pcbench-rp2040-dmxsun_baseboard_4slots | main | 1 | 0 | 0 | 0 | 4 |
| pcbench-rp2040-dmxsun_baseboard_4slots | neckdown | 1 | 0 | 0 | 0 | 4 |
| pcbench-rs485-moist-sensor_adapter-por | main | 1 | 0 | 3 | 0 | 0 |
| pcbench-rs485-moist-sensor_adapter-por | neckdown | 1 | 0 | 3 | 0 | 0 |
| pcbench-rs485-moist-sensor_rs485-moist-sensor | main | 1 | 1 | 0 | 0 | 0 |
| pcbench-rs485-moist-sensor_rs485-moist-sensor | neckdown | 1 | 1 | 0 | 0 | 0 |
| pcbench-split-pcb-throughole_splanck throughhole | main | 1 | 0 | 0 | 0 | 2 |
| pcbench-split-pcb-throughole_splanck throughhole | neckdown | 1 | 0 | 0 | 0 | 2 |
| pcbench-taira-keyboard_tairakb | main | 1 | 8 | 10 | 0 | 0 |
| pcbench-taira-keyboard_tairakb | neckdown | 1 | 8 | 10 | 0 | 0 |
| pcbench-thatmicpre_thatmicpre_v1 | main | 1 | 0 | 0 | 0 | 4 |
| pcbench-thatmicpre_thatmicpre_v1 | neckdown | 1 | 0 | 0 | 0 | 4 |
| pcbench-type5_type5 | main | 1 | 0 | 0 | 0 | 13 |
| pcbench-type5_type5 | neckdown | 1 | 0 | 0 | 0 | 13 |
| pcbench-uC3Moy_uC3Moy | main | 1 | 0 | 0 | 0 | 9 |
| pcbench-uC3Moy_uC3Moy | neckdown | 1 | 0 | 0 | 0 | 9 |
| pcbench-uSKY_uSKY | main | 1 | 25 | 24 | 0 | 0 |
| pcbench-uSKY_uSKY | neckdown | 1 | 25 | 24 | 0 | 0 |

## Warnings

- kicad-issue180-test--test: baseline has 1 judged repetition(s); noise is unmeasured
- kicad-issue184-motorizedopener--motorizedopener: baseline has 1 judged repetition(s); noise is unmeasured
- kicad-issue269-min_fr_test--min_fr_test: baseline has 1 judged repetition(s); noise is unmeasured
- kicad-issue269-noviasonpowerplanes--issue269-noviasonpowerplanes: baseline has 1 judged repetition(s); noise is unmeasured
- kicad-issue269-nowiresonpowerlayers--proba: baseline has 1 judged repetition(s); noise is unmeasured
- kicad-issue283-unconnectedtracesunderpads--natural_tone_preamp: baseline has 1 judged repetition(s); noise is unmeasured
- kicad-issue367-ultraflactyl--ultraflactyl: baseline has 1 judged repetition(s); noise is unmeasured
- kicad-issue368-corneyislandwireless--corney_island_wireless: baseline has 1 judged repetition(s); noise is unmeasured
- kicad-issue558-dev-board-autoroute-demo--dev-board: baseline has 1 judged repetition(s); noise is unmeasured
- kicad-issue632-miniautopilot--mini auto pilot: baseline has 1 judged repetition(s); noise is unmeasured
- kicad-issue742-tastexx-pcb--tastexx-pcb: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-1-Wire-Wing-pcb_1-Wire_Wing: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-12v-automatic-ups_ups-12v: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-16x12-bits-I2C_I2C_Servo: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-1Bitsy_1bitsy: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-2d_conduction_sk9822-matrix: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-4N35-TTL-Serial-Optoisolator_4N35-TTL-Serial-Optoisolator: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-655_testboard: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-6N137-TTL-Serial-Optoisolator_6N137-TTL-Serial-Optoisolator: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-6volt-5W-solar-cc_6vleadacidsolar: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-74Logic_SA_ADC_SA-ADC: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-8bit-cpu_arduino_eeprom_programmer: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-8bit-cpu_programming_interface: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-96boards-sensors_Sensors: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-ABOVISP_ABOVISP: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-ADC-PCM4202-SE_ADC-PCM4202-SE: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-AIOsense_AIOsense: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-APC_AtariPunkConsole: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-APM-RPi-Shield_APM-RPi-Shield: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-AS5043-Encoder_sensor-board: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-ATmega32_ExploreUltraAvrDevKit_40pin_AVRMCU: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-ATtiny461Breakout_ATTiny461DevBoard: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-AVR-ISP_level-shifter_AVR-ISP_level-shifter: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-AVR-ISP_pogo-plug_1.27mm_AVR-ISP_pogo-plug_1.27mm: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-AVR-Playground_hello_world: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-AVR-ZIF-Programmer_AVR-ZIF-Prog: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Amiga-2000-EATX_TICK_OSC_KiCAD_TICK: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Amiga-A1012-PCB_Amiga-A1012: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-AmpOne_dev-AmpOne: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-AnalogThermometer_AnalogThermometer: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Apple-M0110-BT_Apple M0110: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Arduino-Theremin_arduino-theremin-v1: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Arduino_Lipo_Storage_Discharger_Lipo_Storage_Discharger: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Atmel-ICE-Header-Adapter_ice header adapter pcb: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Avem_Hardware_Avem_demo: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-AzizLight_AzizLight: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-BB-PWR-3608_BB-PWR-3608_revA: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-BB-PWR-8009_BB-PWR-8009_revA: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-BLDC-controller_BLDC_controller: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-BML-Badges_BML-Badges: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-BML-Badges_BML_01: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Baofeng-Interface_BaofengInterfaceIsolated: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-BirdAttractor_BirdAttractor_RevA: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-BirdAttractor_BirdAttractor_RevC: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-BirthdayCakeKeyboard_10Key: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Biscay_Blueeye_mcu: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Biscay_Blueeye_sipm-fpga: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Blink-Eras_AVR_ISP_Pogo: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Blitz_.C68: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Blitz_Rev.K_C68: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Blitz_copy: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Box0-hv-analog-breakoutboard_breakout: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Brushless_ESC_Brushless_ESC: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-C-BISCUIT_crowbar: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CAL430FR_CAL430F: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CAL430FR_CAL430F_watch: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_4CH_Mixer: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_APC_Eurorack_Main: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_Abakus_Control: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_Abakus_Main: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_Arduino_VCO_Main: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_Baby_8: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_Buffered_Multiple_Main: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_Buffered_Multiple_SMD_Main: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_CGS_Analog_Switch_Matrix_Control: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_CGS_Analog_Switch_Matrix_Main: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_Classic_ADSR_Main: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_Clock_Divider_Control: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_Clock_Divider_Main: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_Dual_VCA_2_Control: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_Dual_VCA_2_Main: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_Envelope_Follower_Main: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_HAGIWO_6Ch_Gate_Sequencer_Main: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_HAGIWO_MultiOut_Control: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_HAGIWO_MultiOut_Main: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_HAGIWO_Ring_Modulator_Main: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_LFO_Main: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_MFOS_Eight_Stage_Phase_Shifter_-_Control: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_MFOS_Eight_Stage_Phase_Shifter_-_Main: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_MFOS_Noise_Cornucopia_Control: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_MFOS_Noise_Cornucopia_Main_1: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_MFOS_Noise_Cornucopia_Main_2: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_Main_Rectifier_Main: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_Manual_Gate_Control: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_Manual_Gate_Main: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_Power_Modul_Main: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_S_H_Main: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_Single_Attenuator: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_Slim_Precision_Adder_Main: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_Slimline_VCA_Main: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_Slimline_Voltage_Controlled_Switch_-_Main: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_Vactrol_VCF_-_Main: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_YuSynth_Dual_Balanced_Modulator_Main: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CATs-Eurosynth_YuSynth_Improved_Steiner_VCF_Main: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CapPCB_CapPcb: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Cherry-Mx-Bitboard_Cherry Mx Bitboard: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-ChirpHardware_chirp: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CompactFlashBreakout_CompactFlashBreakout: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CoreOne-xCORE200-Original_CoreOne: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CubeSAT-Reaction-Wheel_Edison_Motor_Servo: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-CubeSAT-Reaction-Wheel__autosave-GPIO to motor: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Curryboard_Curryboard: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-DAC-ADAU1966_DAC-ADAU1966: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-DC25_DC25: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-DIYDAC_DIYDAC: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-DPS-1200FB_Adapter_Adapter: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-DaWeather---Project__autosave-CarteDaWeather: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-DasBlinkinput_Das Blinkinput: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Dekada_dekada: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Dekada_dekada_TopoR_curves: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-DerKnopf_digi-pot: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-DerKnopf_led-ring: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-DerKnopf_power-supply: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-DiscoDanceFloorV1_DiscoDongle: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-DonCon2040_DonConPad: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Doorman_doorman_slot: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-DoroidOscillo-Board_Android_Oscilloscope: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-DualLM317BenchSupply_DualLM317BenchSupply: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-DustSensorShield_DustSensorShield: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-E202VAR-Natural-Radio-Receiver_e202var-vlf-radio-receiver: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-EEGFrontier_EEGFrontier: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-ESP-12-breakout_ESP12E-breakout: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-ESP-Breakout_ESP-Breakout: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-ESP07-Breakout_ESP07-Breakout: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-ESP32-Module-Breakout_ESP32S-breakout: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-ESP8266-MQTT-battery-monitor-hw_battery-monitor: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-ESPLux_Board: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-ESP_BaPoTeSta_ESP_BaPoTeSta: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-ESP_WiFiSwitch_WifiSwitch: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Eggbot-Spherebot-polargraph-Controller_eggbot-spherebot-polargraph-controller: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Electronics-MainBoard_MainBoard: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-EncoderBoard_Enc_Pan_Led: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-EnvOpenPico_EliteMicro2040: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-EuroPi_europi-surface-mount: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-FRM16_Relay_Module_I2C_Controller_relay_controller: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-FT231X_breakout_FTDI_FT231XS-U_Breakout: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Feather-ICE40-PCB_feather_ice40: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-FlashProgrammer_flash_programmer: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-FogDrive_attiny45_slim: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-GameTiger_GameTiger: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-HES-V2__autosave-hes: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-HES-V2_hes: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-HW-AC-Emeter_ac-power-monitor: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Hangul-Clock_Hangul: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Hardware-done-with-kicad_AVRlearn: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Hardware_Playground_BL_PCB_latest: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Hardware_Playground_Touch_Switch_1ch_PCB: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Hardware_Playground_buck_led_driver: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Hardware_Playground_esp8266_uno_relay: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Hardware_Playground_hy_adapter: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Hardware_Playground_minimal_node_rfm69w: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Hardware_Playground_nrf52832_uno: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Hardware_Playground_orange_pi_zero_node: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Hardware_Playground_pro_mini: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Hardware_Playground_rpi_zero: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Hardware_Playground_serial_gw_ATMEGA328P: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Hardware_Playground_serial_gw_maple_mini: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Hardware_Playground_usb_shield: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Hardware_Playground_wifi_lights: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-HaveSome_PCB_HaveSomePCB: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-HellScribe_HellScribe: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-HillhacksLantern_LEDLantern: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Hubble_12_bit_analog_out: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Hubble_16_bit_analog_out: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Hubble_jacks: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Hubble_leds: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Hubble_mux: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Hypfer-RGB-W-LED-Controller_led_strip_controller: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-I2CTempsensor_sensors: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-ID-FIX_scanConnect: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-IR-Transponder-ATTiny85-v2_Transponder_v2: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-ISO-port_ch340-usb-serial-isolated: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Inhibition_amplifier: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Inkjet_InkjetBreakout: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Inkjet_InkjetDriver: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Inkjet_PiezoDriver: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Inkjet__autosave-InkjetDriver: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-JLink-SWD_JLink-SWD: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Kefersender_UKW TX: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Keyboard_PCB_Keyboard: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-KiCad-LTC6802-2_main: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-KosselHotendBoard_KosselHotendPCB: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-L6235-PCB_L6235: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-LAUNCHXL-F28027-isolation-PCB_project1: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-LED-Square_PT4115_LED-Square_PT4115: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-LPC2148_Stick_LPC2148_stick: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-LPC2148_Stick__autosave-LPC2148_stick: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-LT3652EvalBoard_LT3652EvalBoard: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-LVDS2TMDS_LVDS2TMDS: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-LadyBugShield_LBS-TEST1: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-LadybugLiteBlue_HW_LadybugBlueLite: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-LiFePO4-Charge-Controller_LiFePO4-Charge-Controller: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Librecalc-Hardware__autosave-calculator: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-LimitSwitchesPlugin_LimitSwitchesPlugin: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-LittleArduinoProjects_LEDx16_board: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-LittleArduinoProjects_sevensegment_led_display_module: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-LoRaCatTrack_GPSLoRa: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-LoRaPP_loramod: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-LongPixel_AnalogDriverMini: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-MAGFest-2017-Swadges_magfest_badges: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-MAVRIC_Hardware_ArduinoPracticeBoard: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-MAVRIC_Hardware_Motherboard: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-MAVRIC_Hardware_SoilBoard: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-MAVRIC_Hardware__autosave-Motherboard: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-MSGEQ7-Breakout-Board_MSGEQ7_Breakout_Board: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Mechaduino-DR_Mechaduino DR 1.01: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Minitel_bbb-adapter: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Minitel_driver_board: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-MixSID_mixsid: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Mouse_Mouse: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-MySRaspiGW_MySRaspiGW: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-MySRaspiGW_MySRaspiGW_PA_LNA: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-MySRaspiGW_MySRaspiGW_PA_LNA_Pimoroni: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-MySRaspiGW_MySRaspiGW_Pimoroni: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-NRC2016_banked_ram: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-NRC2016_usb_sio: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-NRC2016_z80: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-NavigationThing_NavigationThing: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-NeoWall_NeoWall: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Neptune-Hardware_DataAcquisitionBoard: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-NiMH-Charger_NiMH Charger: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-OLD-Stepper-motor-board-design-project_Stepper motor driver: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-OSHW-reCamera-Series_reCamera_Basically_Board: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Omega2-Berrydock_berrydock-mini: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Omega2-mini-dock_Omega2 mini-dock: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-OpAmpPassXsistorBenchSupply_OpAmpPassXsistorBenchSupply: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Open-Source-Power-Supply_PowerSupply_PCB: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Open-Source-Power-Supply_PowerSupply_PCB_backup: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-OpenHardwareExG_ActiveElectrode_OpenHardwareExG_ActiveElectrode: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-OpenHardwareExG_Shield_OpenHardwareExG_Shield: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-OpenHardwareExG_Shield_OpenHardwareExG_Shield_Test_Board: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-OpenHardwareExG_Shield_OpenHardwareExG_Shield_Test_Board_all_panelled: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-OpenVNAVI_driver unit: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-OpenVNAVI_motor unit: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Own-Mailbox-Hardware_eth: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Own-Mailbox-Hardware_mailbox: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-PCB_constant_current_ac_hv: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-PCB_serie_led_strip: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-PCB_small_halogen_replacement: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-PGA2311_pga2311: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-POV_POV: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-PWRmeter_PWMeter: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Paperino_HW_Paperino_shield: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Paperino_HW_paperino_breakout: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Patternflow_patternflow: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Phased-Array-Microphone-using-FPGA_SateliteMicrophone: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Pi1541io_Pi1541io: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Pi5_PCIe_Pi5_PCIe: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-PixyWirelessShield_Shield PIXY: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-PmodHDMIIn_PmodHDMIIn: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-PocketBone_pocketbone-kicad: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Practicas-Curso-Kicad_Ejercicio_2: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Practicas-Curso-Kicad_Salguero_Federico2: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Prototyping_Workshop_Prototyping_PCB: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-PsuFanController_FanController: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-QRPCard_QRPCard: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-R1002_R1002: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-RC2014_RC2014 IDE: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-RC2014_RC2014 RAM: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-RC2014_RC2014 Tandy Sound Card: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-RC6502-Apple-1-Replica_RC6502_Apple_1_SBC: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-RC6502-Apple-1-Replica_RC6502_Terminal: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-RC6502-Apple-1-Replica_RC6502_VDU: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-RFM69HCW_ATSHA204A_Breakout_RFM69HCW_ATSHA204A_Breakout: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-RGBMatrixPanelCPLD-PhotonBackpack_RGBMatrixPanel_CPLD_negative: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-RGBMatrixPanelCPLD-PhotonBackpack_RGBMatrixPanel_CPLD_positive: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-RPi-PWM-Fan-interface_RPi PWM Fan interface: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-RX5808_diversityModule: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-RX5808_rx5808_4button: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Raspberry-Pi-Soft-Power-Controller_Switching Supply TPS563208 MCI: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Raspberry-Pi-Soft-Power-Controller_Zero Current Soft Power: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-ReSDMAC_ReSDMAC: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-ReST32_ReST RRD-FGC-Adapter: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-ReST32_ReST SD-Module: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Retro1DecodingModules_AddressDecoderModule: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-RetroWiFiModem_RetroWiFiModem: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-RoBoC_CameraAdaptor: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-RoBoC_RoboticsMKII: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-S1G-Mod_JST_Adapter: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-S4A-Mini-board_s4a-mini-board: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-SMDBreakouts_smd_breakout: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-SMDBreakouts_smd_breakout_quad: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-SNAP-Badge_SNAP_badge: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-SOICbite_SOICbite_SWD: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-STM32F303_LQFP48_STM32_LQFP48: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-STM32F373_LQFP48_STM32_LQFP48: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Shift-in-32-HC165_shift-in: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Shift-out-32-HC595_Shift-out: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-SimpleCPLD_SimpleCPLD: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-SmartLaserCO2-PCB_LaserPointer: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-SmartLaserCO2-PCB_OptAdjust: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-SmartLaserCO2-PCB_WaterCool: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Solare-BQ24210_Solare-BQ24210: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-SparkSwitch_SparkProtectionSwitch: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Starburst-One_alpha: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Starling_Starling_V1: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Starling__autosave-Starling WiPSU ver_0.1: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-SynthDrumTrigger_Synth Drum Trigger: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-TB6600StepperDriver_DEW_TB6600-V1: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-TK44_TK44: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-TLPHnodeV2_TLPHnodeV2: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-TMC261-stepstick_TMC261-stepstick-v1.1: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-TX5823_TX5823: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Teensy-3.5-Breakout-Boaard_PropShield_Uno: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Teensy-3.5-Breakout-Boaard_TeensyMegaIoShield: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Teensy-3.5-Breakout-Boaard_Test: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Teensy-Hats_Teensy-7-Segment-Hat: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Teensy-Hats_Teensy-LCD-LiDAR-Hat: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-TeensyProtoboard_TeensyProtoboard: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-ThinkerShield_ThinkerShield: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-TinyTracker_ub-minimal: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-ToslinkCNC_Toslink PlanetCNC ECO shield: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-ToslinkCNC__autosave-ToslinkCNC_OneAxis: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-ToslinkCNC_toslink_arduino_shield: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-TripleDelay2399_TripleDelay2399: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-ULPI-Pmod_ULPI-Pmod: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-UProgrammer-Hardware_Programmer: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-UltraPIF_Hardware_led: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-UltraPIF_Hardware_pif_adapter: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-UltrasonicSystem_Schematic: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-UniversalBoard4Nucleo_Nucleo_Universal_Board: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Usb-Serial-Breakout-Cp2102_cp2102: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-VC4000MultiROM_MultiRomCard: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-VM-sensor-PT1000_vm-sensor-pt100: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Ventilator_indicator-board: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Ventilator_pressure_XGZP6897A: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Ventilator_pressure_mpx5700ap_gp: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Ventilator_pressure_mpxv5004_10dp: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Ventilator_pressure_mpxv5004dp: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Ventilator_pressure_mpxv5010dp: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Ventilator_pressure_ms4525do: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-WHCS-Base-Station_base-station: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-WS2811LEDMatrix_matrixcontrol: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-WeatherSpot_vreg_pressure: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-WordClock_v2.0_WordClock_v2: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-Youyue-858D-plus-MCU-adapter_youyue-858d-plus-mcu-adapter: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-abus-cfa1000-display-grabber_acs-display-grabber: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-aciduino_aciduino_pcb: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-airqualitystation_hardware: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-akuhei_akuhei: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-alu_gate_xnor_2in: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-amalthea_amalthea_rev0: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-analog_esr_meter_esr_meter_rev_a: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-anima_MotorDrive: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-antdroid-board_antdroid-board: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-apa102lantern_apa102-lantern-side: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-arduino-led-driver_arduino-led-driver: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-arduino_arduino leds: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-atmegax8-protoboard_atmegax8-protoboard: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-atmel-programmer_atmel_programmer: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-audio_relay_input_switch_relay_switch: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-audprog_audprog_v2: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-autohat-board_inverted-usd-adapter: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-autohat-board_usd-adapter: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-avr-fuser-32_adapter: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-avr_ledprojector_avr_ledprojection: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-avr_ledprojector_avr_ledprojection-0402: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-azalea_azalea: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-badge2016_Badge_init: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-balena-rover-wide-hat_resin-rover: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-basic_esp_board_basic_esp_board: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-beast-phat_beast-phat: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-bee-light-measurement-matrix_bee-light-measurement-matrix: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-beer-gauge_sensorboard: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-beryl_rain_beryl_rain: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-beyblock20_beyblock20: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-bikedar_bikedar: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-blackmagic-isolated_mmp: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-bldc-gimbal-1d_gimbal-board: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-blinky-badge_blinky: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-bms-8s50-ic_bms-8s50-ic: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-bmw-ibus-bluetooth_bmw_bt_cdcemu_analog: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-bmw-ibus-bluetooth_bmw_bt_cdcemu_digital: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-board_armjtag_pmod_compatible_armjtag-pmod: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-boards_shift-register-demo-v2: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-boatcontrol_CommonCathode60A: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-boatcontrol_NonLatchingNO30A: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-bobc_LCD-panel-adapter-lvc: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-bobc_MS-F100: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-bobc_led_clock: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-bobc_matrix_clock: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-bpnode-bb_BPnode-BB: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-breakout-boards_50-to-100: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-breakout-boards_avr-isp-x2: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-breakout-boards_esp8266-jtag: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-breakout-boards_swd-and-uart: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-breakout-boards_swd-to-wires: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-bristle_bot_light_follow_bristle_bot: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-busblaster-to-swd_busblaster-to-swd: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-bypass_crossmix_bypass_crossmix: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-can_firewall_hardware_CAN_Firewall: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-cdm324_backpack_cdm324: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-ciurlys_ciurlys: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-clock_lcdb4: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-cnlohr_wiflier: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-cnlohr_wiflier_B: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-continuity-tester_continuity-tester: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-cookiecutter-xsproduct_{{cookiecutter.product_name}}: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-crossover-schiit-stack_xover4schiit: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-custom_cpu--ALU_custom_cpu--ALU: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-custom_cpu--register_custom_cpu--register: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-data-manager_data-manager: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-decelerator4030_decelerator4030: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-denbit_basic: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-deskbot_breakout: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-devttys0_IRis: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-digital_clock_led_clock_3_and_4_digit: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-digital_clock_led_clock_v1: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-disco-dongle_DiscoDongle: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-divergence_meter_dm_control: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-domotics_base-board-arranged: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-dorkyboard_keyboard: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-drawduino_drawduino: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-dust_sensor_dust_sensor: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-dustbox_Dustbox: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-eBUS-Adapter_Groeger: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-eeg_brainboard_batteryv0: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-eink-adapter_eink: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-epaper-102_epaper-102: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-epapercard_epapercard: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-ergo-snm-keyboard_receiver: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-esp-leipa_esp-12: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-esp-serial-terminal_esp-com: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-esp12-appliance_mod_esp12-appliance-mod: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-esp12-breakout_ESP12Breakout: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-esp32-4-channel-relays_esp32-4-channel-relays: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-esp32-ethernet_esp32-ethernet: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-esp32stack_esp32stack: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-esp8266-for-uppatvind_Air-Purifier-Uppatvind: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-esp8266_32x32panel_esp_12_f_595: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-esp8266_32x32panel_esp_12_f_595_ORDERED: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-esp8266_envmonitor_environment-monitor: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-esp8266_envmonitor_environment-monitor-1.2: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-esp8266_envmonitor_environment-monitor-1.4: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-esp8266_link_test_esp_micro85-only: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-esp8266_network_speaker_esp_network_speaker: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-esp8266_wi07_3_adapter_esp: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-espalarm_alarm: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-esper_EsperDNS: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-esper_programmer: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-espeverywhere__autosave-espeverywhere_breakout: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-espionage_esplight: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-everled_everled: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-ezusb-logicanalyzer_cypress_logic_analyzer: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-f.60_keyboard: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-fan_controller_fan_controller: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-fifogfx_c64cart: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-filament_extruder_sensor: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-fingerprint-with-esp32_quet van tay: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-firefly-jar_solar_lamp: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-fp2_extension_sample_fp2_usb_breakout: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-free-of-charge_BMS: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-freeDSP-CLASSIC-SMD-BALANCED_FreeDSP_BAL: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-freeUSBi_USBi_Programmer: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-ftdi-jtag-programmer_JTAGProgrammer: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-gb-hardware_GB-BRK-M-XS: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-gb-hardware_GB-BRK-TR-A: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-gb-hardware_GB-CART256K-A: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-gb-hardware_GB-CART32K-A: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-gb-hardware_GB-CARTPP-XC: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-gb-hardware_GB-LIVE32: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-gb-hardware_GB-MBCTEST: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-gdrom_adapter_board_adapter: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-gepetto_circuito: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-guitar_fret: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-gwurrbus_pwm: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-hackaday_esp-14_power_meter__autosave-esp-14: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-hackpad_orpheuspad_pcb: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-hackyflasher_Flasher: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-hardware-designs_c-trigger: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-hardware-designs_m-trigger: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-hardware-designs_nixie-combo: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-hardware-designs_nixie-power: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-hardware-designs_soil-moisture-sensor: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-hardware-designs_solar-harvester: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-hardware-designs_spsgrf-board: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-hbr-mk2_hbr-mk2-bpfs: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-hbr-mk2_hbr-mk2-digital: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-hbr-mk2_hbr-mk2-lpfs: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-headstage-adapter_headstage adapter: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-helmholtz-servo_CurrentServo: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-hm-mod-rpi-rtc_hm-mod-rpi-rtc: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-hw_trials_demo: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-hwstar_ac-power-monitor: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-icehat_icehat: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-imfr-schematics_Telescopio: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-induction-hob_temperature-sender: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-jadonk_PocketBone: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-jdy-08-board_jdy-08: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-juno-chorus-clone_juno-chorus-clone: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-karabas-nano_karabas-nano-revA: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-karabas-nano_karabas-nano-revB: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-karabas-nano_karabas-nano-revC: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-karabas-nano_karabas-nano-revG: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-karabas-nano_wifi_revA: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kassenautomat.mdb-interface_mdb-interface: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-keyboards_Djinn: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kicad-guitar-preamp_Preamp-Instructables: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kicad-projects_BatCharge: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kicad-projects_ili9341-breakout: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kicad_bbb-melzi: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kika-in-space_DS8500: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kika-in-space_analog-test-board: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kinetoscope_ethernet: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kinetoscope_microcontroller: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kinetoscope_sram-bank: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kit2-led-cube_led_cube: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_12V5A_breakout: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_12_24_boost_converter: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_40-channel-hv-switching-board: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_4_switch_array: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_8_switch_array: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_BQ25570_Harvester: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_CH330: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_CO2: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_DIY_detector: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_Lcr_addon: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_Minisumo_V2.1: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_OSO-BOOK-C1: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_OtterScreen: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_PSLab: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_Potentiometer_mount_4LED: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_Potentiometer_mount_8LED: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_RPi_shield: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_T32_ref: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_USB-C-Screen-Adapter: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_USB-C-Screen-Adapter-LDR6023SS: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace__autosave-nunchuk_breakout: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_aquarius: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_ardfpga: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_beehive: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_dropbot-front-panel: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_dropbot_control_board: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_dynamixel_shield: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_esp8266: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_flypi: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_flypi_v2: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_gas_sensor: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_grove_adaptor: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_hbridge_driver: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_hp_led_switch: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_hum_temp_sensor: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_ideal_diode: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_ir_sensor: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_led_driver: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_level_shifter: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_minisumo_v3: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_nunchuk_breakout: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_peltier: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_piezo_amplifier: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_pmt_combiner: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_power_supply: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_sensor: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_solenoid_driver: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_spike_n_hold: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_sympetrum-v2%20NFF1.1: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_teensy-fx: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_temp_breakout: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_threeboard: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_training_board_v02: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_trans_switch_volt_amp: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_tt_nano_HAT_b1: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_tt_nano_HAT_b2: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-kitspace_tt_opt101_module_b1: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-klangorium_logic_noise_playground: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-komputer-klavier_KomputerKlavier: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-led-wordclock_wordclock: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-led_array_atmega8_led_array: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-lfi-rig_lfi-driver: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-light-painting-wand_light-wand: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-linklayer_contact: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-low-power-counter_lpcounter: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-m2-electronics_m2fc: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-m2-electronics_m2pogo: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-m2-electronics_m2r: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-m2-electronics_m2rl: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-mac-pro-conversion_front-panel-power-adapter: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-magic-table_etch-a-sketch_cyclone: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-makerspace-emonth_resistor_board: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-marlin-neopixel-bridge_ATtiny85_Marneo: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-mavbridge_mavbridge: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-maytal_Maytal: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-mdbwerk_mdbwerk: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-mearm-base-pcb_ServoPCB: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-mechkeys_lfk78-jtag: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-medusa_medusa_rs422_rx: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-memory-display_memory-display: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-memsarray_mems_array: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-microphone_preamp: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-mightyduino_mightyduino: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-mikoto_mikoto-flashbed: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-mini_ice40_mini_ice40: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-miniboard-opamp_miniboard-opamp: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-miniboard-stm32f0_miniboard-stm32f0: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-mobile-sensor-pcb_mobile-sensor-pcb: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-mojo-nes_mojo-nes: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-motor-3xdrv8833-hw_ver1: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-mppt-2420-hc_mppt-2420-hc: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-mppt-2420-hpx_mppt-2420-hpx: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-nRF24breakoutBoard_nRF24-breakout: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-nand_programmer_adapter_tsop48: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-nanoSwinSidC_nanoSwinSidC: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-navelino-leaf_navelino-leaf: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-nextbusclock_NextBusClockV1: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-nfl-led-scoreboard_passive-rpi-hub75-adapter: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-nfl-led-scoreboard_passive3-rpi-hub75-adapter: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-nikon_gps_nikon_gps: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-nixie-clock_ab18x5-breakout: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-nodemcu-backstage_NodeMCU Backstage: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-nodemcu-basecamp_NodeMCU Basecamp: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-nonSNES_SNSP-CPU-1CHIP: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-nrf2rfm69_nrf2rfm69: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-nunchuk_rf_hw_NunchukRF_V3: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-oasis_ledboard: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-oled-bmp280-touch_oled-bmp280-touch: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-one-shift-register_one-shift-register: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-onion2-breakout_onion2 breakout: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-opentilt_opentilt2: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-oshtimer_transponder: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-ottawa-badges-2016_ottawa-badge-tagger-2016: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-ozinverter_ozinverterkicad: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-pcb-covox-amp-v2_pcb-covox-amp-v2: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-pcb-covox-amp_pcb-covox-amp: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-pcb-ks0108-128x64-glcd_circuit: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-pcb-usb-ft245r-parallel-adapter_pcb-usb-ft245r-parallel-adapter: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-pesho_pesho: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-phone_rtty_interface_phone_rtty_rev_a: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-phone_rtty_interface_phone_rtty_rev_b: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-photon_Sprinkler_sprinkler: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-pi-zero-stepper-board_pi-zero-stepper-board: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-pi_plant_MCP3002: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-pico-pi-rel_pico-pi: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-pmw3360-pcb_pmw3360_pcb_jst: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-pocketbone-kicad_pocketbone-kicad: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-polypoint_pinpoint_timebase: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-ponyser-pcb_Ponyser: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-preamp-two_input-selector: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-preamp-two_mcu-board: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-preamp-two_mdac-attenuator: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-prog-cc-100mA_prog-cc-100mA: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-project-hydra-meshtastic-pcb_meshtastic-diy: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-pulse_v1_pulse: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-pusheenz40_sadcatz40: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-pwm-2420-lus_pwm-2420-lus: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-radio_antenna-iridium: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-raspberry_pi_pullup_button_pullup_shutdown_button: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-raspberry_pi_pullup_button_pullup_shutdown_button(revB): baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-rc2014_bank_switcher_z80_cpm_mmu: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-real-time-chess_kfchess: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-recalbox-gpio-board__autosave-board: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-recalbox-gpio-board_board: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-retrocon_bbb-adapter: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-retrocon_driver_board: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-retroreflectors_TANGOFLOCK: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-rfcx-sentinel-pcb_Mainboard: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-rfidBoard_rfid: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-rgb-led_rgb-led-v2: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-rgb-strip-controller__autosave-rgb-strip: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-rgb2ypbpr_rgb2ypbpr: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-rjw57_cpu-board: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-roomba-ESP12E_roomba-esp: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-rotary-encoder-breakout_rotary-encoder-breakout: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-royer_royer: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-rp2040-dmxsun_baseboard_2slots: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-rp2040-dmxsun_baseboard_4slots: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-rs485-moist-sensor_adapter-por: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-rs485-moist-sensor_interconnect: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-rs485-moist-sensor_rs485-moist-sensor: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-rufs__autosave-simple_kicad_schema_and_pcb_v1: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-rufs_aprs_tracker: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-rufs_dra818v_breakout_board: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-rufs_simple_kicad_schema_and_pcb_v1: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-rufs_smart_psu: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-rufs_spv1040_power_controller: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-rxadc_14_rxadc_14: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-saiboard_3x8: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-saiboard_8x3: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-scimpy_amp: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-scimpy_crossover: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-scimpy_powersupply: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-scimpy_volumebuffer: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-sensorboard_DiffIR: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-sensorboard_DiffIR.kicad_pcb_narrow: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-shutter_Shutter V4: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-shutter_speed_tester_shutter_speed_tester: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-simplebus2-intercom_repeater_v2: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-sms-cart-32k_cart: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-smt-zvs-driver_IH10-sl: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-snappi-zero_snappi-zero: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-soil-moisture-sensor-analog_analog-moist-sensor: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-solar-lanterns_proto1: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-sonic3_feram_adapter_sonic3_feram_adapter: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-spisolator_spisolator: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-split-pcb-throughole_splanck throughhole: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-srambo_1_srambo_1: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-ssr-wifi_adapter: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-starfish_starfish: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-starsynctrackers_reset_switch: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-stlinkv2_breakout_stlink_breakout: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-stm32_ccd_camera_ccd: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-stubby_hex: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-sv650sds_sds_tool: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-taira-keyboard_tairakb: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-tbd_tbd: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-tdstat_TDstatv2: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-technoshield-ui-hw_technoshield: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-teensy-touch_teensy-touch: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-teensy-weather-badge_teensyi2c: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-teensy-wifi-weather-logger_teensyi2c: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-temperature-alarm_controller: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-tepmachcha_tepmachcha: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-tessel-ice40__autosave-project: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-thatmicpre_thatmicpre_v1: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-thatmicpre_thatmicpre_v2: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-thegrid_thegrid: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-thingBot-LoRa_thingBot-LoRa_v1P0: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-thingBot-LoRa_thingBot-LoRa_v1P1: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-timecircuits-hardware_BTTF-TimeCircuits: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-tiny-8088_Computer: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-tinyFISH_tinyBRUSH: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-tinyisp-micro_tinyispmicro: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-tinymuseum_museum: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-type5_type5: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-uC3Moy_uC3Moy: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-uSKY_uSKY: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-uext-esp32_UEXT_ESP32: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-usb_rs232c_usb_rs232c_rev_a: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-vatx_vatx: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-vdcmon_vdcmon: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-wavegen_rev3: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-wavegen_waveform-generator: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-wavegen_waveform-generator-rev1: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-wavegen_wavegen: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-wifiLCD_wifilcd: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-xmasOrn_xmasOrn: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-xwhatits-capsense-controller_model-f-3178-adaptor: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-z2amiller_sensorboard_programmer: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-zx-sizif-128_sizif128: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-zx-sizif-512-ext_sizif512ext: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-zx-sizif-512-wifi_sizif512-wifi: baseline has 1 judged repetition(s); noise is unmeasured
- pcbench-zx-sizif-xxs_sizif-xxs: baseline has 1 judged repetition(s); noise is unmeasured
