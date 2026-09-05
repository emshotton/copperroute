# Comparison: java-current vs rs-main

Created 2026-09-05T23:47:07+00:00 from runs java-278fe14, v2-eval. Config: threads=1 jobs=12 max_passes=10 timeout=300s seeds=1 time metric = cpu_s tier=pcbench

| candidate | version | sha |
|---|---|---|
| java-current | current | `278fe14123c4` |
| rs-main | — | `70f681ac6770` |

## Overall

- **rs-main** vs java-current: **worse** — 523 wins, 82 losses (31 on hard metrics), 0 ties

## Per tier

| tier | candidate | wins | losses | ties | clean-pass rate | median Δscore | median time ratio |
|---|---|---|---|---|---|---|---|
| all | rs-main | 523 | 82 | 0 | 0.71 | +10.0 | 0.14 |
| d3-a | rs-main | 116 | 27 | 0 | 0.83 | +0.0 | 0.03 |
| d3-b | rs-main | 227 | 33 | 0 | 0.81 | +6.7 | 0.11 |
| d3-c | rs-main | 180 | 22 | 0 | 0.49 | +36.0 | 0.39 |
| pcbench | rs-main | 523 | 82 | 0 | 0.71 | +10.0 | 0.14 |

## Per board

| board | referee | candidate | clean | unrouted | viol | score | Δscore | noise | cpu s | Δcpu s | vias | length mm | verdict |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| pcbench-1-Wire-Wing-pcb_1-Wire_Wing | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 46.5 | | 18 | 474.1 | baseline |
| pcbench-1-Wire-Wing-pcb_1-Wire_Wing | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 4.2 | -42.3 | 8 | 474.8 | win (score) |
| pcbench-16x12-bits-I2C_I2C_Servo | kicad | java-current | 0.00 | 0 | 53 | 905.4 | | unmeasured | 53.3 | | 46 | 1039.0 | baseline |
| pcbench-16x12-bits-I2C_I2C_Servo | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +94.6 | | 11.2 | -42.1 | 19 | 1075.1 | win (clean_pass_rate) |
| pcbench-1Bitsy_1bitsy | kicad | java-current | 0.00 | 0 | 55 | 931.2 | | unmeasured | 355.2 | | 70 | 1042.8 | baseline |
| pcbench-1Bitsy_1bitsy | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +68.8 | | 165.0 | -190.2 | 59 | 1039.6 | win (clean_pass_rate) |
| pcbench-2d_conduction_sk9822-matrix | kicad | java-current | 0.00 | 2 | 0 | 994.6 | | unmeasured | 354.9 | | 106 | 2776.3 | baseline |
| pcbench-2d_conduction_sk9822-matrix | kicad | rs-main | 0.00 | 1 | 0 | 997.3 | +2.7 | | 300.0 | -54.9 | 74 | 2861.0 | win (unrouted) |
| pcbench-4N35-TTL-Serial-Optoisolator_4N35-TTL-Serial-Optoisolator | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 17.4 | | 0 | 219.9 | baseline |
| pcbench-4N35-TTL-Serial-Optoisolator_4N35-TTL-Serial-Optoisolator | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.4 | -17.1 | 0 | 207.4 | win (score) |
| pcbench-655_testboard | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 71.0 | | 27 | 1002.1 | baseline |
| pcbench-655_testboard | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 24.6 | -46.4 | 18 | 1007.9 | win (score) |
| pcbench-6N137-TTL-Serial-Optoisolator_6N137-TTL-Serial-Optoisolator | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 44.9 | | 0 | 277.3 | baseline |
| pcbench-6N137-TTL-Serial-Optoisolator_6N137-TTL-Serial-Optoisolator | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 1.9 | -43.0 | 0 | 267.8 | win (score) |
| pcbench-6volt-5W-solar-cc_6vleadacidsolar | kicad | java-current | 0.00 | 0 | 12 | 980.2 | | unmeasured | 94.8 | | 19 | 949.6 | baseline |
| pcbench-6volt-5W-solar-cc_6vleadacidsolar | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +19.8 | | 26.7 | -68.1 | 11 | 956.1 | win (clean_pass_rate) |
| pcbench-74Logic_SA_ADC_SA-ADC | kicad | java-current | 0.00 | 74 | 0 | 834.8 | | unmeasured | 383.7 | | 175 | 2981.5 | baseline |
| pcbench-74Logic_SA_ADC_SA-ADC | kicad | rs-main | 0.00 | 3 | 0 | 993.3 | +158.5 | | 300.2 | -83.5 | 174 | 4501.1 | win (unrouted) |
| pcbench-96boards-sensors_Sensors | kicad | java-current | 0.00 | 14 | 8 | 943.7 | | unmeasured | 394.2 | | 136 | 3334.9 | baseline |
| pcbench-96boards-sensors_Sensors | kicad | rs-main | 0.00 | 2 | 9 | 986.3 | +42.6 | | 187.8 | -206.4 | 122 | 3754.7 | win (unrouted) |
| pcbench-ABOVISP_ABOVISP | kicad | java-current | 0.00 | 1 | 7 | 922.6 | | unmeasured | 55.1 | | 5 | 196.6 | baseline |
| pcbench-ABOVISP_ABOVISP | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +77.4 | | 1.6 | -53.5 | 2 | 204.3 | win (clean_pass_rate) |
| pcbench-ADC-PCM4202-SE_ADC-PCM4202-SE | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 288.0 | | 41 | 2707.0 | baseline |
| pcbench-ADC-PCM4202-SE_ADC-PCM4202-SE | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 229.8 | -58.2 | 35 | 2595.6 | win (score) |
| pcbench-APC_AtariPunkConsole | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 29.2 | | 2 | 479.1 | baseline |
| pcbench-APC_AtariPunkConsole | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 2.5 | -26.7 | 0 | 453.4 | win (score) |
| pcbench-APM-RPi-Shield_APM-RPi-Shield | kicad | java-current | 0.00 | 4 | 2 | 964.8 | | unmeasured | 203.5 | | 31 | 1346.1 | baseline |
| pcbench-APM-RPi-Shield_APM-RPi-Shield | kicad | rs-main | 0.00 | 7 | 1 | 942.4 | -22.4 | | 112.9 | -90.6 | 23 | 1202.2 | loss (unrouted) |
| pcbench-AS5043-Encoder_sensor-board | kicad | java-current | 0.00 | 0 | 2 | 986.7 | | unmeasured | 32.3 | | 9 | 249.7 | baseline |
| pcbench-AS5043-Encoder_sensor-board | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +13.3 | | 1.4 | -30.9 | 4 | 252.7 | win (clean_pass_rate) |
| pcbench-ATmega32_ExploreUltraAvrDevKit_40pin_AVRMCU | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 75.8 | | 7 | 1189.3 | baseline |
| pcbench-ATmega32_ExploreUltraAvrDevKit_40pin_AVRMCU | kicad | rs-main | 0.00 | 1 | 0 | 987.5 | -12.5 | | 42.4 | -33.4 | 3 | 1151.9 | loss (clean_pass_rate) |
| pcbench-ATtiny461Breakout_ATTiny461DevBoard | kicad | java-current | 0.00 | 1 | 2 | 963.2 | | unmeasured | 65.3 | | 3 | 296.4 | baseline |
| pcbench-ATtiny461Breakout_ATTiny461DevBoard | kicad | rs-main | 0.00 | 0 | 1 | 994.7 | +31.6 | | 0.6 | -64.7 | 0 | 325.8 | win (unrouted) |
| pcbench-AVR-ISP_level-shifter_AVR-ISP_level-shifter | kicad | java-current | 0.00 | 1 | 4 | 972.7 | | unmeasured | 83.2 | | 8 | 366.9 | baseline |
| pcbench-AVR-ISP_level-shifter_AVR-ISP_level-shifter | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +27.3 | | 5.0 | -78.2 | 1 | 372.3 | win (clean_pass_rate) |
| pcbench-AVR-ISP_pogo-plug_1.27mm_AVR-ISP_pogo-plug_1.27mm | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 44.2 | | 1 | 48.8 | baseline |
| pcbench-AVR-ISP_pogo-plug_1.27mm_AVR-ISP_pogo-plug_1.27mm | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 0.5 | -43.8 | 1 | 64.5 | loss (score) |
| pcbench-AVR-Playground_hello_world | kicad | java-current | 0.00 | 1 | 0 | 800.0 | | unmeasured | 26.6 | | 0 | 108.5 | baseline |
| pcbench-AVR-Playground_hello_world | kicad | rs-main | 0.00 | 1 | 0 | 800.0 | 0.0 | | 0.2 | -26.4 | 0 | 108.5 | win (cpu_s) |
| pcbench-AVR-ZIF-Programmer_AVR-ZIF-Prog | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 104.0 | | 8 | 1490.5 | baseline |
| pcbench-AVR-ZIF-Programmer_AVR-ZIF-Prog | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 38.9 | -65.1 | 0 | 1468.9 | win (score) |
| pcbench-Amiga-A1012-PCB_Amiga-A1012 | kicad | java-current | 0.00 | 0 | 42 | 913.4 | | unmeasured | 107.4 | | 47 | 1745.1 | baseline |
| pcbench-Amiga-A1012-PCB_Amiga-A1012 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +86.6 | | 38.9 | -68.5 | 33 | 1723.3 | win (clean_pass_rate) |
| pcbench-AmpOne_dev-AmpOne | kicad | java-current | 0.00 | 1 | 0 | 996.9 | | unmeasured | 293.2 | | 60 | 2688.2 | baseline |
| pcbench-AmpOne_dev-AmpOne | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +3.1 | | 178.3 | -114.9 | 33 | 2777.6 | win (clean_pass_rate) |
| pcbench-AnalogThermometer_AnalogThermometer | kicad | java-current | 0.00 | 0 | 3 | 982.9 | | unmeasured | 36.6 | | 2 | 208.7 | baseline |
| pcbench-AnalogThermometer_AnalogThermometer | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +17.1 | | 2.0 | -34.6 | 2 | 214.9 | win (clean_pass_rate) |
| pcbench-Apple-M0110-BT_Apple M0110 | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 155.5 | | 0 | 4384.2 | baseline |
| pcbench-Apple-M0110-BT_Apple M0110 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 5.5 | -150.0 | 0 | 4356.1 | win (score) |
| pcbench-Arduino-Theremin_arduino-theremin-v1 | kicad | java-current | 0.00 | 1 | 0 | 963.0 | | unmeasured | 50.1 | | 3 | 596.6 | baseline |
| pcbench-Arduino-Theremin_arduino-theremin-v1 | kicad | rs-main | 0.00 | 1 | 0 | 963.0 | +0.0 | | 19.3 | -30.8 | 3 | 581.4 | win (score) |
| pcbench-Arduino_Lipo_Storage_Discharger_Lipo_Storage_Discharger | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 64.0 | | 7 | 1041.8 | baseline |
| pcbench-Arduino_Lipo_Storage_Discharger_Lipo_Storage_Discharger | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 12.4 | -51.6 | 6 | 1061.4 | win (score) |
| pcbench-Atmel-ICE-Header-Adapter_ice header adapter pcb | kicad | java-current | 0.00 | 3 | 0 | 957.7 | | unmeasured | 137.7 | | 0 | 970.9 | baseline |
| pcbench-Atmel-ICE-Header-Adapter_ice header adapter pcb | kicad | rs-main | 0.00 | 1 | 0 | 985.9 | +28.2 | | 134.9 | -2.8 | 0 | 1000.8 | win (unrouted) |
| pcbench-Avem_Hardware_Avem_demo | kicad | java-current | 0.00 | 1 | 39 | 890.0 | | unmeasured | 148.2 | | 16 | 656.7 | baseline |
| pcbench-Avem_Hardware_Avem_demo | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +110.0 | | 36.7 | -111.5 | 12 | 707.9 | win (clean_pass_rate) |
| pcbench-AzizLight_AzizLight | kicad | java-current | 0.00 | 0 | 23 | 954.9 | | unmeasured | 93.0 | | 35 | 722.5 | baseline |
| pcbench-AzizLight_AzizLight | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +45.1 | | 37.8 | -55.3 | 10 | 737.9 | win (clean_pass_rate) |
| pcbench-BB-PWR-3608_BB-PWR-3608_revA | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 49.1 | | 3 | 94.3 | baseline |
| pcbench-BB-PWR-3608_BB-PWR-3608_revA | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.5 | -48.6 | 3 | 92.9 | win (score) |
| pcbench-BB-PWR-8009_BB-PWR-8009_revA | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 59.9 | | 2 | 72.3 | baseline |
| pcbench-BB-PWR-8009_BB-PWR-8009_revA | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.6 | -59.3 | 0 | 76.7 | win (score) |
| pcbench-BLDC-controller_BLDC_controller | kicad | java-current | 0.00 | 134 | 46 | 674.5 | | unmeasured | 357.1 | | 44 | 863.4 | baseline |
| pcbench-BLDC-controller_BLDC_controller | kicad | rs-main | 0.00 | 20 | 1 | 954.1 | +279.5 | | 140.9 | -216.2 | 47 | 1253.8 | win (unrouted) |
| pcbench-BML-Badges_BML-Badges | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 27.0 | | 0 | 460.8 | baseline |
| pcbench-BML-Badges_BML-Badges | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 0.4 | -26.6 | 0 | 470.8 | loss (score) |
| pcbench-BML-Badges_BML_01 | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 19.9 | | 0 | 339.9 | baseline |
| pcbench-BML-Badges_BML_01 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.9 | -19.0 | 0 | 334.9 | win (score) |
| pcbench-Baofeng-Interface_BaofengInterfaceIsolated | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 67.2 | | 0 | 668.8 | baseline |
| pcbench-Baofeng-Interface_BaofengInterfaceIsolated | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 30.9 | -36.3 | 0 | 647.2 | win (score) |
| pcbench-BirdAttractor_BirdAttractor_RevA | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 54.6 | | 2 | 374.0 | baseline |
| pcbench-BirdAttractor_BirdAttractor_RevA | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 1.3 | -53.3 | 2 | 364.7 | win (score) |
| pcbench-BirdAttractor_BirdAttractor_RevC | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 68.6 | | 41 | 1085.6 | baseline |
| pcbench-BirdAttractor_BirdAttractor_RevC | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 31.4 | -37.3 | 20 | 1185.5 | win (score) |
| pcbench-BirthdayCakeKeyboard_10Key | kicad | java-current | 0.00 | 0 | 62 | 935.1 | | unmeasured | 269.9 | | 72 | 3894.3 | baseline |
| pcbench-BirthdayCakeKeyboard_10Key | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +64.9 | | 298.9 | +29.0 | 67 | 3906.0 | win (clean_pass_rate) |
| pcbench-Biscay_Blueeye_mcu | kicad | java-current | 0.00 | 0 | 17 | 946.9 | | unmeasured | 81.0 | | 7 | 1169.7 | baseline |
| pcbench-Biscay_Blueeye_mcu | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +53.1 | | 19.4 | -61.5 | 1 | 1251.5 | win (clean_pass_rate) |
| pcbench-Biscay_Blueeye_sipm-fpga | kicad | java-current | 0.00 | 2 | 77 | 884.0 | | unmeasured | 349.4 | | 26 | 1423.5 | baseline |
| pcbench-Biscay_Blueeye_sipm-fpga | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +116.0 | | 155.9 | -193.5 | 13 | 1406.3 | win (clean_pass_rate) |
| pcbench-Blink-Eras_AVR_ISP_Pogo | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 13.5 | | 0 | 97.0 | baseline |
| pcbench-Blink-Eras_AVR_ISP_Pogo | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.1 | -13.4 | 0 | 97.0 | win (score) |
| pcbench-Box0-hv-analog-breakoutboard_breakout | kicad | java-current | 0.00 | 128 | 0 | 174.2 | | unmeasured | 254.5 | | 0 | 98.2 | baseline |
| pcbench-Box0-hv-analog-breakoutboard_breakout | kicad | rs-main | 0.00 | 128 | 0 | 174.2 | 0.0 | | 65.8 | -188.7 | 0 | 98.2 | win (cpu_s) |
| pcbench-Brushless_ESC_Brushless_ESC | kicad | java-current | 0.00 | 1 | 69 | 908.6 | | unmeasured | 171.9 | | 74 | 1377.5 | baseline |
| pcbench-Brushless_ESC_Brushless_ESC | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +91.4 | | 97.7 | -74.2 | 27 | 1455.0 | win (clean_pass_rate) |
| pcbench-C-BISCUIT_crowbar | kicad | java-current | 0.00 | 0 | 3 | 966.7 | | unmeasured | 22.2 | | 5 | 145.5 | baseline |
| pcbench-C-BISCUIT_crowbar | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +33.3 | | 0.9 | -21.3 | 1 | 139.8 | win (clean_pass_rate) |
| pcbench-CAL430FR_CAL430F | kicad | java-current | 0.00 | 8 | 28 | 852.2 | | unmeasured | 375.7 | | 63 | 1012.4 | baseline |
| pcbench-CAL430FR_CAL430F | kicad | rs-main | 0.00 | 6 | 0 | 934.8 | +82.6 | | 169.9 | -205.8 | 76 | 1108.6 | win (unrouted) |
| pcbench-CAL430FR_CAL430F_watch | kicad | java-current | 0.00 | 0 | 18 | 836.4 | | unmeasured | 52.7 | | 9 | 389.8 | baseline |
| pcbench-CAL430FR_CAL430F_watch | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +163.6 | | 3.5 | -49.1 | 1 | 354.9 | win (clean_pass_rate) |
| pcbench-CapPCB_CapPcb | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 26.4 | | 0 | 85.3 | baseline |
| pcbench-CapPCB_CapPcb | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.1 | -26.3 | 0 | 77.3 | win (score) |
| pcbench-Cherry-Mx-Bitboard_Cherry Mx Bitboard | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 12.8 | | 0 | 72.5 | baseline |
| pcbench-Cherry-Mx-Bitboard_Cherry Mx Bitboard | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.1 | -12.7 | 0 | 72.5 | win (cpu_s) |
| pcbench-ChirpHardware_chirp | kicad | java-current | 0.00 | 1 | 65 | 860.0 | | unmeasured | 202.8 | | 31 | 763.5 | baseline |
| pcbench-ChirpHardware_chirp | kicad | rs-main | 0.00 | 1 | 0 | 990.0 | +130.0 | | 90.0 | -112.8 | 26 | 765.6 | win (violations) |
| pcbench-CompactFlashBreakout_CompactFlashBreakout | kicad | java-current | 0.00 | 0 | 1 | 993.9 | | unmeasured | 97.3 | | 16 | 479.8 | baseline |
| pcbench-CompactFlashBreakout_CompactFlashBreakout | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +6.1 | | 28.9 | -68.4 | 15 | 470.8 | win (clean_pass_rate) |
| pcbench-CoreOne-xCORE200-Original_CoreOne | kicad | java-current | 0.00 | 302 | 3 | 629.2 | | unmeasured | 366.1 | | 269 | 2530.7 | baseline |
| pcbench-CoreOne-xCORE200-Original_CoreOne | kicad | rs-main | 0.00 | 21 | 1 | 974.0 | +344.9 | | 302.1 | -64.0 | 298 | 6863.9 | win (unrouted) |
| pcbench-CubeSAT-Reaction-Wheel_Edison_Motor_Servo | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 16.5 | | 0 | 115.7 | baseline |
| pcbench-CubeSAT-Reaction-Wheel_Edison_Motor_Servo | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.1 | -16.4 | 0 | 115.7 | win (cpu_s) |
| pcbench-CubeSAT-Reaction-Wheel__autosave-GPIO to motor | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 66.0 | | 3 | 673.2 | baseline |
| pcbench-CubeSAT-Reaction-Wheel__autosave-GPIO to motor | kicad | rs-main | 0.00 | 1 | 0 | 963.0 | -37.0 | | 6.9 | -59.1 | 0 | 635.8 | loss (clean_pass_rate) |
| pcbench-Curryboard_Curryboard | kicad | java-current | 0.00 | 3 | 0 | 980.3 | | unmeasured | 366.6 | | 79 | 1825.5 | baseline |
| pcbench-Curryboard_Curryboard | kicad | rs-main | 0.00 | 0 | 2 | 997.4 | +17.1 | | 208.0 | -158.6 | 50 | 1916.5 | win (unrouted) |
| pcbench-DAC-ADAU1966_DAC-ADAU1966 | kicad | java-current | 0.00 | 102 | 0 | 836.3 | | unmeasured | 374.5 | | 129 | 3150.1 | baseline |
| pcbench-DAC-ADAU1966_DAC-ADAU1966 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +163.7 | | 300.5 | -74.0 | 122 | 3968.4 | win (clean_pass_rate) |
| pcbench-DC25_DC25 | kicad | java-current | 0.00 | 0 | 25 | 927.5 | | unmeasured | 133.2 | | 46 | 1506.4 | baseline |
| pcbench-DC25_DC25 | kicad | rs-main | 0.00 | 0 | 9 | 973.9 | +46.4 | | 74.1 | -59.1 | 18 | 1531.7 | win (violations) |
| pcbench-DIYDAC_DIYDAC | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 17.8 | | 0 | 68.1 | baseline |
| pcbench-DIYDAC_DIYDAC | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.1 | -17.7 | 0 | 68.1 | win (cpu_s) |
| pcbench-DPS-1200FB_Adapter_Adapter | kicad | java-current | 0.00 | 1 | 3 | 987.2 | | unmeasured | 88.0 | | 0 | 796.0 | baseline |
| pcbench-DPS-1200FB_Adapter_Adapter | kicad | rs-main | 0.00 | 1 | 2 | 988.8 | +1.6 | | 53.4 | -34.6 | 0 | 772.6 | win (violations) |
| pcbench-DaWeather---Project__autosave-CarteDaWeather | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 32.4 | | 0 | 594.9 | baseline |
| pcbench-DaWeather---Project__autosave-CarteDaWeather | kicad | rs-main | 0.00 | 0 | 1 | 995.3 | -4.7 | | 0.6 | -31.8 | 0 | 600.6 | loss (clean_pass_rate) |
| pcbench-DasBlinkinput_Das Blinkinput | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 340.6 | | 20 | 652.1 | baseline |
| pcbench-DasBlinkinput_Das Blinkinput | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 64.1 | -276.5 | 16 | 673.4 | win (score) |
| pcbench-Dekada_dekada | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 148.2 | | 0 | 379.8 | baseline |
| pcbench-Dekada_dekada | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 1.4 | -146.7 | 0 | 379.6 | win (score) |
| pcbench-Dekada_dekada_TopoR_curves | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 146.5 | | 0 | 379.8 | baseline |
| pcbench-Dekada_dekada_TopoR_curves | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 1.1 | -145.5 | 0 | 379.6 | win (score) |
| pcbench-DerKnopf_digi-pot | kicad | java-current | 0.00 | 1 | 54 | 821.2 | | unmeasured | 204.7 | | 29 | 462.4 | baseline |
| pcbench-DerKnopf_digi-pot | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +178.8 | | 22.3 | -182.4 | 16 | 453.8 | win (clean_pass_rate) |
| pcbench-DerKnopf_led-ring | kicad | java-current | 0.00 | 0 | 4 | 988.4 | | unmeasured | 206.5 | | 45 | 569.4 | baseline |
| pcbench-DerKnopf_led-ring | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +11.6 | | 107.2 | -99.3 | 35 | 543.1 | win (clean_pass_rate) |
| pcbench-DerKnopf_power-supply | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 70.1 | | 1 | 185.7 | baseline |
| pcbench-DerKnopf_power-supply | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 1.0 | -69.2 | 0 | 191.4 | win (score) |
| pcbench-DiscoDanceFloorV1_DiscoDongle | kicad | java-current | 0.00 | 0 | 11 | 958.5 | | unmeasured | 284.7 | | 10 | 519.2 | baseline |
| pcbench-DiscoDanceFloorV1_DiscoDongle | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +41.5 | | 32.2 | -252.4 | 6 | 508.3 | win (clean_pass_rate) |
| pcbench-DoroidOscillo-Board_Android_Oscilloscope | kicad | java-current | 0.00 | 13 | 67 | 914.6 | | unmeasured | 336.3 | | 102 | 1636.5 | baseline |
| pcbench-DoroidOscillo-Board_Android_Oscilloscope | kicad | rs-main | 0.00 | 5 | 0 | 983.8 | +69.3 | | 220.6 | -115.7 | 87 | 1794.3 | win (unrouted) |
| pcbench-DualLM317BenchSupply_DualLM317BenchSupply | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 101.3 | | 0 | 1139.7 | baseline |
| pcbench-DualLM317BenchSupply_DualLM317BenchSupply | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 7.1 | -94.2 | 0 | 1086.7 | win (score) |
| pcbench-DustSensorShield_DustSensorShield | kicad | java-current | 0.00 | 1 | 0 | 941.2 | | unmeasured | 69.4 | | 6 | 467.2 | baseline |
| pcbench-DustSensorShield_DustSensorShield | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +58.8 | | 8.7 | -60.7 | 9 | 475.7 | win (clean_pass_rate) |
| pcbench-E202VAR-Natural-Radio-Receiver_e202var-vlf-radio-receiver | kicad | java-current | 0.00 | 1 | 0 | 989.1 | | unmeasured | 77.6 | | 1 | 961.2 | baseline |
| pcbench-E202VAR-Natural-Radio-Receiver_e202var-vlf-radio-receiver | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +10.9 | | 29.8 | -47.9 | 0 | 948.5 | win (clean_pass_rate) |
| pcbench-ESP-12-breakout_ESP12E-breakout | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 47.7 | | 9 | 310.7 | baseline |
| pcbench-ESP-12-breakout_ESP12E-breakout | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 2.4 | -45.3 | 3 | 338.4 | win (score) |
| pcbench-ESP-Breakout_ESP-Breakout | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 41.5 | | 18 | 350.8 | baseline |
| pcbench-ESP-Breakout_ESP-Breakout | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 6.8 | -34.7 | 17 | 340.0 | win (score) |
| pcbench-ESP07-Breakout_ESP07-Breakout | kicad | java-current | 0.00 | 2 | 0 | 969.2 | | unmeasured | 116.8 | | 10 | 702.2 | baseline |
| pcbench-ESP07-Breakout_ESP07-Breakout | kicad | rs-main | 0.00 | 1 | 0 | 984.6 | +15.4 | | 32.2 | -84.6 | 7 | 729.2 | win (unrouted) |
| pcbench-ESP32-Module-Breakout_ESP32S-breakout | kicad | java-current | 0.00 | 0 | 13 | 943.5 | | unmeasured | 83.6 | | 27 | 750.3 | baseline |
| pcbench-ESP32-Module-Breakout_ESP32S-breakout | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +56.5 | | 22.8 | -60.9 | 24 | 770.9 | win (clean_pass_rate) |
| pcbench-ESP8266-MQTT-battery-monitor-hw_battery-monitor | kicad | java-current | 0.00 | 0 | 27 | 943.2 | | unmeasured | 87.5 | | 25 | 645.7 | baseline |
| pcbench-ESP8266-MQTT-battery-monitor-hw_battery-monitor | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +56.8 | | 32.6 | -54.8 | 14 | 700.8 | win (clean_pass_rate) |
| pcbench-ESPLux_Board | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 270.2 | | 10 | 789.9 | baseline |
| pcbench-ESPLux_Board | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 9.8 | -260.4 | 8 | 827.4 | win (score) |
| pcbench-ESP_BaPoTeSta_ESP_BaPoTeSta | kicad | java-current | 0.00 | 2 | 0 | 981.5 | | unmeasured | 166.0 | | 32 | 791.0 | baseline |
| pcbench-ESP_BaPoTeSta_ESP_BaPoTeSta | kicad | rs-main | 0.00 | 6 | 0 | 944.4 | -37.0 | | 90.2 | -75.7 | 22 | 733.9 | loss (unrouted) |
| pcbench-ESP_WiFiSwitch_WifiSwitch | kicad | java-current | 0.00 | 0 | 1 | 990.9 | | unmeasured | 37.6 | | 6 | 323.1 | baseline |
| pcbench-ESP_WiFiSwitch_WifiSwitch | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +9.1 | | 6.5 | -31.1 | 1 | 348.1 | win (clean_pass_rate) |
| pcbench-Eggbot-Spherebot-polargraph-Controller_eggbot-spherebot-polargraph-controller | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 56.5 | | 3 | 1268.6 | baseline |
| pcbench-Eggbot-Spherebot-polargraph-Controller_eggbot-spherebot-polargraph-controller | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 13.5 | -43.0 | 0 | 1266.0 | win (score) |
| pcbench-Electronics-MainBoard_MainBoard | kicad | java-current | 0.00 | 1 | 42 | 967.0 | | unmeasured | 336.6 | | 84 | 2899.8 | baseline |
| pcbench-Electronics-MainBoard_MainBoard | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +33.0 | | 299.3 | -37.3 | 28 | 2967.5 | win (clean_pass_rate) |
| pcbench-EncoderBoard_Enc_Pan_Led | kicad | java-current | 0.00 | 7 | 26 | 874.2 | | unmeasured | 349.3 | | 38 | 840.4 | baseline |
| pcbench-EncoderBoard_Enc_Pan_Led | kicad | rs-main | 0.00 | 3 | 4 | 960.8 | +86.6 | | 115.0 | -234.3 | 40 | 992.9 | win (unrouted) |
| pcbench-FRM16_Relay_Module_I2C_Controller_relay_controller | kicad | java-current | 0.00 | 0 | 3 | 989.1 | | unmeasured | 302.2 | | 52 | 1478.1 | baseline |
| pcbench-FRM16_Relay_Module_I2C_Controller_relay_controller | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +10.9 | | 24.3 | -277.9 | 14 | 1518.7 | win (clean_pass_rate) |
| pcbench-FT231X_breakout_FTDI_FT231XS-U_Breakout | kicad | java-current | 0.00 | 3 | 3 | 910.0 | | unmeasured | 130.3 | | 17 | 371.3 | baseline |
| pcbench-FT231X_breakout_FTDI_FT231XS-U_Breakout | kicad | rs-main | 0.00 | 1 | 0 | 975.0 | +65.0 | | 34.0 | -96.4 | 13 | 387.5 | win (unrouted) |
| pcbench-FlashProgrammer_flash_programmer | kicad | java-current | 0.00 | 5 | 52 | 890.8 | | unmeasured | 331.7 | | 60 | 1453.3 | baseline |
| pcbench-FlashProgrammer_flash_programmer | kicad | rs-main | 0.00 | 2 | 0 | 985.8 | +95.0 | | 193.4 | -138.3 | 43 | 1586.1 | win (unrouted) |
| pcbench-FogDrive_attiny45_slim | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 17.9 | | 0 | 52.5 | baseline |
| pcbench-FogDrive_attiny45_slim | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.0 | -17.8 | 0 | 52.5 | win (cpu_s) |
| pcbench-HES-V2__autosave-hes | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 30.7 | | 0 | 456.9 | baseline |
| pcbench-HES-V2__autosave-hes | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.4 | -30.3 | 0 | 456.2 | win (score) |
| pcbench-HES-V2_hes | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 30.0 | | 0 | 456.9 | baseline |
| pcbench-HES-V2_hes | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.3 | -29.7 | 0 | 456.2 | win (score) |
| pcbench-HW-AC-Emeter_ac-power-monitor | kicad | java-current | 0.00 | 1 | 13 | 966.3 | | unmeasured | 157.8 | | 37 | 1110.7 | baseline |
| pcbench-HW-AC-Emeter_ac-power-monitor | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +33.6 | | 84.3 | -73.5 | 12 | 1296.4 | win (clean_pass_rate) |
| pcbench-Hangul-Clock_Hangul | kicad | java-current | 0.00 | 0 | 28 | 973.2 | | unmeasured | 274.8 | | 73 | 3221.7 | baseline |
| pcbench-Hangul-Clock_Hangul | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +26.8 | | 299.7 | +25.0 | 37 | 3469.8 | win (clean_pass_rate) |
| pcbench-Hardware-done-with-kicad_AVRlearn | kicad | java-current | 0.00 | 0 | 3 | 976.0 | | unmeasured | 29.9 | | 8 | 415.6 | baseline |
| pcbench-Hardware-done-with-kicad_AVRlearn | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +24.0 | | 2.0 | -27.9 | 0 | 398.6 | win (clean_pass_rate) |
| pcbench-Hardware_Playground_BL_PCB_latest | kicad | java-current | 0.00 | 0 | 24 | 979.9 | | unmeasured | 158.5 | | 16 | 1888.5 | baseline |
| pcbench-Hardware_Playground_BL_PCB_latest | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +20.1 | | 38.0 | -120.5 | 10 | 1863.8 | win (clean_pass_rate) |
| pcbench-Hardware_Playground_Touch_Switch_1ch_PCB | kicad | java-current | 0.00 | 1 | 19 | 955.1 | | unmeasured | 285.4 | | 29 | 668.1 | baseline |
| pcbench-Hardware_Playground_Touch_Switch_1ch_PCB | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +44.9 | | 61.6 | -223.8 | 20 | 702.6 | win (clean_pass_rate) |
| pcbench-Hardware_Playground_buck_led_driver | kicad | java-current | 0.00 | 1 | 0 | 944.4 | | unmeasured | 34.8 | | 3 | 104.1 | baseline |
| pcbench-Hardware_Playground_buck_led_driver | kicad | rs-main | 0.00 | 1 | 0 | 944.4 | +0.0 | | 1.4 | -33.4 | 3 | 92.8 | win (score) |
| pcbench-Hardware_Playground_esp8266_uno_relay | kicad | java-current | 0.00 | 1 | 0 | 994.9 | | unmeasured | 116.6 | | 1 | 580.2 | baseline |
| pcbench-Hardware_Playground_esp8266_uno_relay | kicad | rs-main | 0.00 | 1 | 0 | 994.9 | +0.0 | | 50.4 | -66.2 | 0 | 591.9 | win (score) |
| pcbench-Hardware_Playground_hy_adapter | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 67.3 | | 7 | 73.1 | baseline |
| pcbench-Hardware_Playground_hy_adapter | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 1.5 | -65.7 | 5 | 60.5 | win (score) |
| pcbench-Hardware_Playground_minimal_node_rfm69w | kicad | java-current | 0.00 | 9 | 8 | 897.1 | | unmeasured | 263.8 | | 22 | 473.1 | baseline |
| pcbench-Hardware_Playground_minimal_node_rfm69w | kicad | rs-main | 0.00 | 6 | 1 | 939.8 | +42.7 | | 93.0 | -170.8 | 30 | 559.6 | win (unrouted) |
| pcbench-Hardware_Playground_nrf52832_uno | kicad | java-current | 0.00 | 4 | 36 | 883.3 | | unmeasured | 280.3 | | 27 | 1402.3 | baseline |
| pcbench-Hardware_Playground_nrf52832_uno | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +116.7 | | 80.2 | -200.1 | 27 | 1701.2 | win (clean_pass_rate) |
| pcbench-Hardware_Playground_orange_pi_zero_node | kicad | java-current | 0.00 | 1 | 2 | 993.1 | | unmeasured | 153.1 | | 1 | 781.5 | baseline |
| pcbench-Hardware_Playground_orange_pi_zero_node | kicad | rs-main | 0.00 | 1 | 1 | 994.1 | +1.0 | | 43.5 | -109.6 | 1 | 779.2 | win (violations) |
| pcbench-Hardware_Playground_pro_mini | kicad | java-current | 0.00 | 1 | 0 | 987.2 | | unmeasured | 131.0 | | 4 | 462.4 | baseline |
| pcbench-Hardware_Playground_pro_mini | kicad | rs-main | 0.00 | 1 | 0 | 987.2 | -0.0 | | 135.1 | +4.1 | 6 | 459.9 | loss (score) |
| pcbench-Hardware_Playground_rpi_zero | kicad | java-current | 0.00 | 2 | 21 | 971.4 | | unmeasured | 193.6 | | 11 | 845.8 | baseline |
| pcbench-Hardware_Playground_rpi_zero | kicad | rs-main | 0.00 | 0 | 11 | 989.9 | +18.4 | | 152.5 | -41.0 | 5 | 835.7 | win (unrouted) |
| pcbench-Hardware_Playground_serial_gw_ATMEGA328P | kicad | java-current | 0.00 | 1 | 57 | 928.7 | | unmeasured | 338.0 | | 44 | 779.4 | baseline |
| pcbench-Hardware_Playground_serial_gw_ATMEGA328P | kicad | rs-main | 0.00 | 1 | 16 | 975.9 | +47.1 | | 161.6 | -176.4 | 29 | 740.1 | win (violations) |
| pcbench-Hardware_Playground_serial_gw_maple_mini | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 125.3 | | 7 | 925.2 | baseline |
| pcbench-Hardware_Playground_serial_gw_maple_mini | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 62.0 | -63.3 | 2 | 906.4 | win (score) |
| pcbench-Hardware_Playground_usb_shield | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 126.7 | | 0 | 703.6 | baseline |
| pcbench-Hardware_Playground_usb_shield | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 147.5 | +20.8 | 0 | 697.9 | win (score) |
| pcbench-Hardware_Playground_wifi_lights | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 70.7 | | 8 | 628.4 | baseline |
| pcbench-Hardware_Playground_wifi_lights | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 5.9 | -64.8 | 4 | 602.2 | win (score) |
| pcbench-HaveSome_PCB_HaveSomePCB | kicad | java-current | 0.00 | 1 | 0 | 956.5 | | unmeasured | 66.7 | | 0 | 74.2 | baseline |
| pcbench-HaveSome_PCB_HaveSomePCB | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +43.5 | | 1.6 | -65.0 | 0 | 85.4 | win (clean_pass_rate) |
| pcbench-HellScribe_HellScribe | kicad | java-current | 0.00 | 0 | 25 | 946.2 | | unmeasured | 105.8 | | 47 | 1541.4 | baseline |
| pcbench-HellScribe_HellScribe | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +53.8 | | 23.3 | -82.5 | 20 | 1555.1 | win (clean_pass_rate) |
| pcbench-HillhacksLantern_LEDLantern | kicad | java-current | 0.00 | 2 | 0 | 928.6 | | unmeasured | 52.0 | | 2 | 379.4 | baseline |
| pcbench-HillhacksLantern_LEDLantern | kicad | rs-main | 0.00 | 1 | 0 | 964.3 | +35.7 | | 13.1 | -38.8 | 0 | 402.0 | win (unrouted) |
| pcbench-Hypfer-RGB-W-LED-Controller_led_strip_controller | kicad | java-current | 0.00 | 1 | 0 | 986.5 | | unmeasured | 88.3 | | 0 | 1093.1 | baseline |
| pcbench-Hypfer-RGB-W-LED-Controller_led_strip_controller | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +13.5 | | 8.9 | -79.4 | 0 | 1174.6 | win (clean_pass_rate) |
| pcbench-I2CTempsensor_sensors | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 16.2 | | 0 | 142.5 | baseline |
| pcbench-I2CTempsensor_sensors | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 0.2 | -16.0 | 0 | 152.9 | loss (score) |
| pcbench-ID-FIX_scanConnect | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 12.5 | | 0 | 50.5 | baseline |
| pcbench-ID-FIX_scanConnect | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 0.0 | -12.5 | 0 | 50.8 | loss (score) |
| pcbench-IR-Transponder-ATTiny85-v2_Transponder_v2 | kicad | java-current | 0.00 | 1 | 0 | 952.4 | | unmeasured | 31.4 | | 3 | 130.4 | baseline |
| pcbench-IR-Transponder-ATTiny85-v2_Transponder_v2 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +47.6 | | 2.7 | -28.7 | 1 | 146.4 | win (clean_pass_rate) |
| pcbench-ISO-port_ch340-usb-serial-isolated | kicad | java-current | 0.00 | 1 | 0 | 987.3 | | unmeasured | 101.5 | | 20 | 661.3 | baseline |
| pcbench-ISO-port_ch340-usb-serial-isolated | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +12.7 | | 25.9 | -75.6 | 4 | 728.5 | win (clean_pass_rate) |
| pcbench-Inhibition_amplifier | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 243.2 | | 9 | 2386.8 | baseline |
| pcbench-Inhibition_amplifier | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 15.3 | -227.8 | 5 | 2372.5 | win (score) |
| pcbench-Inkjet_InkjetBreakout | kicad | java-current | 0.00 | 0 | 21 | 676.9 | | unmeasured | 27.0 | | 3 | 252.6 | baseline |
| pcbench-Inkjet_InkjetBreakout | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +323.1 | | 1.7 | -25.4 | 2 | 256.9 | win (clean_pass_rate) |
| pcbench-Inkjet_InkjetDriver | kicad | java-current | 0.00 | 0 | 6 | 981.5 | | unmeasured | 83.1 | | 28 | 1179.4 | baseline |
| pcbench-Inkjet_InkjetDriver | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +18.5 | | 40.5 | -42.5 | 20 | 1150.7 | win (clean_pass_rate) |
| pcbench-Inkjet_PiezoDriver | kicad | java-current | 0.00 | 1 | 9 | 903.4 | | unmeasured | 61.4 | | 9 | 265.7 | baseline |
| pcbench-Inkjet_PiezoDriver | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +96.6 | | 31.9 | -29.4 | 6 | 277.7 | win (clean_pass_rate) |
| pcbench-Inkjet__autosave-InkjetDriver | kicad | java-current | 0.00 | 0 | 6 | 981.5 | | unmeasured | 85.0 | | 28 | 1179.4 | baseline |
| pcbench-Inkjet__autosave-InkjetDriver | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +18.5 | | 28.7 | -56.3 | 20 | 1150.7 | win (clean_pass_rate) |
| pcbench-JLink-SWD_JLink-SWD | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 44.7 | | 4 | 298.0 | baseline |
| pcbench-JLink-SWD_JLink-SWD | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 6.9 | -37.7 | 2 | 302.2 | win (score) |
| pcbench-Kefersender_UKW TX | kicad | java-current | 0.00 | 6 | 8 | 912.6 | | unmeasured | 135.6 | | 0 | 416.9 | baseline |
| pcbench-Kefersender_UKW TX | kicad | rs-main | 0.00 | 5 | 0 | 942.5 | +29.9 | | 149.1 | +13.5 | 0 | 408.2 | win (unrouted) |
| pcbench-Keyboard_PCB_Keyboard | kicad | java-current | 0.00 | 260 | 37 | 499.2 | | unmeasured | 336.8 | | 155 | 6645.1 | baseline |
| pcbench-Keyboard_PCB_Keyboard | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +500.7 | | 300.1 | -36.7 | 55 | 12041.2 | win (clean_pass_rate) |
| pcbench-KiCad-LTC6802-2_main | kicad | java-current | 0.00 | 0 | 47 | 731.4 | | unmeasured | 50.3 | | 7 | 466.3 | baseline |
| pcbench-KiCad-LTC6802-2_main | kicad | rs-main | 0.00 | 0 | 2 | 988.6 | +257.1 | | 4.2 | -46.1 | 4 | 445.5 | win (violations) |
| pcbench-KosselHotendBoard_KosselHotendPCB | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 25.2 | | 0 | 212.9 | baseline |
| pcbench-KosselHotendBoard_KosselHotendPCB | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.4 | -24.8 | 0 | 208.5 | win (score) |
| pcbench-L6235-PCB_L6235 | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 34.4 | | 0 | 715.8 | baseline |
| pcbench-L6235-PCB_L6235 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 3.1 | -31.2 | 0 | 696.5 | win (score) |
| pcbench-LAUNCHXL-F28027-isolation-PCB_project1 | kicad | java-current | 0.00 | 1 | 4 | 948.6 | | unmeasured | 57.3 | | 7 | 175.5 | baseline |
| pcbench-LAUNCHXL-F28027-isolation-PCB_project1 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +51.4 | | 1.2 | -56.1 | 0 | 184.3 | win (clean_pass_rate) |
| pcbench-LED-Square_PT4115_LED-Square_PT4115 | kicad | java-current | 0.00 | 0 | 6 | 980.6 | | unmeasured | 117.8 | | 1 | 471.7 | baseline |
| pcbench-LED-Square_PT4115_LED-Square_PT4115 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +19.4 | | 6.3 | -111.5 | 2 | 460.3 | win (clean_pass_rate) |
| pcbench-LPC2148_Stick_LPC2148_stick | kicad | java-current | 0.00 | 33 | 42 | 820.8 | | unmeasured | 347.7 | | 90 | 2872.8 | baseline |
| pcbench-LPC2148_Stick_LPC2148_stick | kicad | rs-main | 0.00 | 2 | 2 | 989.6 | +168.8 | | 190.3 | -157.5 | 103 | 4431.1 | win (unrouted) |
| pcbench-LPC2148_Stick__autosave-LPC2148_stick | kicad | java-current | 0.00 | 35 | 39 | 814.7 | | unmeasured | 354.9 | | 89 | 2780.0 | baseline |
| pcbench-LPC2148_Stick__autosave-LPC2148_stick | kicad | rs-main | 0.00 | 2 | 2 | 989.6 | +174.9 | | 211.6 | -143.3 | 103 | 4431.1 | win (unrouted) |
| pcbench-LT3652EvalBoard_LT3652EvalBoard | kicad | java-current | 0.00 | 0 | 30 | 915.5 | | unmeasured | 61.4 | | 14 | 596.8 | baseline |
| pcbench-LT3652EvalBoard_LT3652EvalBoard | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +84.5 | | 14.3 | -47.1 | 4 | 610.6 | win (clean_pass_rate) |
| pcbench-LVDS2TMDS_LVDS2TMDS | kicad | java-current | 0.00 | 0 | 32 | 863.8 | | unmeasured | 62.1 | | 16 | 380.0 | baseline |
| pcbench-LVDS2TMDS_LVDS2TMDS | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +136.2 | | 5.3 | -56.8 | 11 | 382.8 | win (clean_pass_rate) |
| pcbench-LadyBugShield_LBS-TEST1 | kicad | java-current | 0.00 | 0 | 8 | 968.6 | | unmeasured | 67.4 | | 15 | 417.6 | baseline |
| pcbench-LadyBugShield_LBS-TEST1 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +31.4 | | 5.8 | -61.6 | 6 | 435.5 | win (clean_pass_rate) |
| pcbench-LadybugLiteBlue_HW_LadybugBlueLite | kicad | java-current | 0.00 | 1 | 0 | 992.9 | | unmeasured | 146.1 | | 42 | 996.3 | baseline |
| pcbench-LadybugLiteBlue_HW_LadybugBlueLite | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +7.1 | | 54.4 | -91.8 | 17 | 1067.6 | win (clean_pass_rate) |
| pcbench-LiFePO4-Charge-Controller_LiFePO4-Charge-Controller | kicad | java-current | 0.00 | 0 | 20 | 964.6 | | unmeasured | 67.6 | | 10 | 772.0 | baseline |
| pcbench-LiFePO4-Charge-Controller_LiFePO4-Charge-Controller | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +35.4 | | 8.4 | -59.2 | 4 | 801.5 | win (clean_pass_rate) |
| pcbench-Librecalc-Hardware__autosave-calculator | kicad | java-current | 0.00 | 173 | 0 | 654.0 | | unmeasured | 371.6 | | 225 | 1763.6 | baseline |
| pcbench-Librecalc-Hardware__autosave-calculator | kicad | rs-main | 0.00 | 3 | 0 | 994.0 | +340.0 | | 299.7 | -71.9 | 246 | 4323.2 | win (unrouted) |
| pcbench-LimitSwitchesPlugin_LimitSwitchesPlugin | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 31.6 | | 0 | 714.1 | baseline |
| pcbench-LimitSwitchesPlugin_LimitSwitchesPlugin | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 0.8 | -30.7 | 2 | 693.2 | loss (score) |
| pcbench-LoRaCatTrack_GPSLoRa | kicad | java-current | 0.00 | 4 | 0 | 948.7 | | unmeasured | 141.7 | | 21 | 1070.6 | baseline |
| pcbench-LoRaCatTrack_GPSLoRa | kicad | rs-main | 0.00 | 2 | 0 | 974.4 | +25.6 | | 86.4 | -55.3 | 14 | 1214.0 | win (unrouted) |
| pcbench-LoRaPP_loramod | kicad | java-current | 0.00 | 0 | 31 | 966.8 | | unmeasured | 351.6 | | 30 | 675.8 | baseline |
| pcbench-LoRaPP_loramod | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +33.2 | | 141.9 | -209.7 | 19 | 693.7 | win (clean_pass_rate) |
| pcbench-LongPixel_AnalogDriverMini | kicad | java-current | 0.00 | 0 | 15 | 933.3 | | unmeasured | 40.7 | | 13 | 360.1 | baseline |
| pcbench-LongPixel_AnalogDriverMini | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +66.7 | | 6.2 | -34.6 | 6 | 375.5 | win (clean_pass_rate) |
| pcbench-MAGFest-2017-Swadges_magfest_badges | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 339.0 | | 30 | 1231.9 | baseline |
| pcbench-MAGFest-2017-Swadges_magfest_badges | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 33.2 | -305.8 | 14 | 1404.8 | win (score) |
| pcbench-MAVRIC_Hardware_ArduinoPracticeBoard | kicad | java-current | 0.00 | 0 | 20 | 923.1 | | unmeasured | 95.9 | | 18 | 713.6 | baseline |
| pcbench-MAVRIC_Hardware_ArduinoPracticeBoard | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +76.9 | | 22.4 | -73.5 | 13 | 706.6 | win (clean_pass_rate) |
| pcbench-MAVRIC_Hardware_Motherboard | kicad | java-current | 0.00 | 0 | 29 | 963.3 | | unmeasured | 154.8 | | 60 | 2108.9 | baseline |
| pcbench-MAVRIC_Hardware_Motherboard | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +36.7 | | 94.2 | -60.7 | 20 | 2125.4 | win (clean_pass_rate) |
| pcbench-MAVRIC_Hardware_SoilBoard | kicad | java-current | 0.00 | 0 | 30 | 846.1 | | unmeasured | 52.5 | | 14 | 358.5 | baseline |
| pcbench-MAVRIC_Hardware_SoilBoard | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +153.8 | | 6.2 | -46.2 | 5 | 388.4 | win (clean_pass_rate) |
| pcbench-MAVRIC_Hardware__autosave-Motherboard | kicad | java-current | 0.00 | 0 | 29 | 963.3 | | unmeasured | 147.8 | | 60 | 2108.9 | baseline |
| pcbench-MAVRIC_Hardware__autosave-Motherboard | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +36.7 | | 112.8 | -35.1 | 20 | 2125.4 | win (clean_pass_rate) |
| pcbench-MSGEQ7-Breakout-Board_MSGEQ7_Breakout_Board | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 20.0 | | 0 | 191.8 | baseline |
| pcbench-MSGEQ7-Breakout-Board_MSGEQ7_Breakout_Board | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.4 | -19.7 | 0 | 189.0 | win (score) |
| pcbench-Mechaduino-DR_Mechaduino DR 1.01 | kicad | java-current | 0.00 | 1 | 76 | 909.0 | | unmeasured | 358.8 | | 69 | 1255.4 | baseline |
| pcbench-Mechaduino-DR_Mechaduino DR 1.01 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +91.0 | | 171.8 | -187.1 | 59 | 1408.3 | win (clean_pass_rate) |
| pcbench-Minitel_bbb-adapter | kicad | java-current | 0.00 | 0 | 5 | 983.0 | | unmeasured | 71.4 | | 3 | 937.7 | baseline |
| pcbench-Minitel_bbb-adapter | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +16.9 | | 4.1 | -67.3 | 2 | 955.9 | win (clean_pass_rate) |
| pcbench-Minitel_driver_board | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 53.0 | | 0 | 874.3 | baseline |
| pcbench-Minitel_driver_board | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 4.6 | -48.4 | 0 | 845.3 | win (score) |
| pcbench-MixSID_mixsid | kicad | java-current | 0.00 | 4 | 0 | 980.6 | | unmeasured | 357.7 | | 38 | 2538.9 | baseline |
| pcbench-MixSID_mixsid | kicad | rs-main | 0.00 | 2 | 0 | 990.3 | +9.7 | | 168.9 | -188.8 | 39 | 2554.8 | win (unrouted) |
| pcbench-MySRaspiGW_MySRaspiGW | kicad | java-current | 0.00 | 3 | 0 | 769.2 | | unmeasured | 71.1 | | 2 | 66.5 | baseline |
| pcbench-MySRaspiGW_MySRaspiGW | kicad | rs-main | 0.00 | 3 | 0 | 769.2 | -0.0 | | 3.4 | -67.6 | 3 | 55.2 | loss (score) |
| pcbench-MySRaspiGW_MySRaspiGW_PA_LNA | kicad | java-current | 0.00 | 2 | 0 | 846.2 | | unmeasured | 60.8 | | 3 | 71.3 | baseline |
| pcbench-MySRaspiGW_MySRaspiGW_PA_LNA | kicad | rs-main | 0.00 | 3 | 0 | 769.2 | -76.9 | | 5.2 | -55.6 | 2 | 62.5 | loss (unrouted) |
| pcbench-MySRaspiGW_MySRaspiGW_PA_LNA_Pimoroni | kicad | java-current | 0.00 | 1 | 0 | 923.1 | | unmeasured | 66.8 | | 3 | 95.5 | baseline |
| pcbench-MySRaspiGW_MySRaspiGW_PA_LNA_Pimoroni | kicad | rs-main | 0.00 | 1 | 0 | 923.1 | +0.0 | | 3.0 | -63.8 | 3 | 87.1 | win (score) |
| pcbench-MySRaspiGW_MySRaspiGW_Pimoroni | kicad | java-current | 0.00 | 1 | 0 | 923.1 | | unmeasured | 58.1 | | 2 | 91.9 | baseline |
| pcbench-MySRaspiGW_MySRaspiGW_Pimoroni | kicad | rs-main | 0.00 | 1 | 0 | 923.1 | -0.0 | | 5.3 | -52.8 | 2 | 98.3 | loss (score) |
| pcbench-NRC2016_banked_ram | kicad | java-current | 0.00 | 14 | 0 | 905.4 | | unmeasured | 336.5 | | 0 | 3479.1 | baseline |
| pcbench-NRC2016_banked_ram | kicad | rs-main | 0.00 | 9 | 0 | 939.2 | +33.8 | | 89.6 | -246.9 | 0 | 3984.5 | win (unrouted) |
| pcbench-NRC2016_usb_sio | kicad | java-current | 0.00 | 4 | 0 | 951.8 | | unmeasured | 198.8 | | 9 | 1666.6 | baseline |
| pcbench-NRC2016_usb_sio | kicad | rs-main | 0.00 | 3 | 0 | 963.9 | +12.0 | | 104.5 | -94.3 | 4 | 2000.8 | win (unrouted) |
| pcbench-NRC2016_z80 | kicad | java-current | 0.00 | 10 | 0 | 965.5 | | unmeasured | 363.4 | | 0 | 5415.3 | baseline |
| pcbench-NRC2016_z80 | kicad | rs-main | 0.00 | 2 | 0 | 993.1 | +27.6 | | 89.6 | -273.9 | 0 | 6118.9 | win (unrouted) |
| pcbench-NavigationThing_NavigationThing | kicad | java-current | 0.00 | 1 | 0 | 988.9 | | unmeasured | 210.6 | | 28 | 794.9 | baseline |
| pcbench-NavigationThing_NavigationThing | kicad | rs-main | 0.00 | 0 | 3 | 993.3 | +4.4 | | 55.0 | -155.6 | 5 | 835.0 | win (unrouted) |
| pcbench-NeoWall_NeoWall | kicad | java-current | 0.00 | 0 | 8 | 950.0 | | unmeasured | 49.9 | | 14 | 572.2 | baseline |
| pcbench-NeoWall_NeoWall | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +50.0 | | 2.4 | -47.5 | 2 | 589.8 | win (clean_pass_rate) |
| pcbench-Neptune-Hardware_DataAcquisitionBoard | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 31.7 | | 0 | 722.7 | baseline |
| pcbench-Neptune-Hardware_DataAcquisitionBoard | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 1.6 | -30.1 | 0 | 683.9 | win (score) |
| pcbench-NiMH-Charger_NiMH Charger | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 52.4 | | 2 | 882.2 | baseline |
| pcbench-NiMH-Charger_NiMH Charger | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 1.6 | -50.8 | 0 | 848.6 | win (score) |
| pcbench-OLD-Stepper-motor-board-design-project_Stepper motor driver | kicad | java-current | 0.00 | 2 | 0 | 956.5 | | unmeasured | 105.9 | | 20 | 505.9 | baseline |
| pcbench-OLD-Stepper-motor-board-design-project_Stepper motor driver | kicad | rs-main | 0.00 | 1 | 0 | 978.3 | +21.7 | | 20.6 | -85.3 | 19 | 514.2 | win (unrouted) |
| pcbench-Omega2-Berrydock_berrydock-mini | kicad | java-current | 0.00 | 1 | 81 | 918.1 | | unmeasured | 376.7 | | 79 | 1607.5 | baseline |
| pcbench-Omega2-Berrydock_berrydock-mini | kicad | rs-main | 0.00 | 1 | 12 | 983.8 | +65.7 | | 137.4 | -239.3 | 72 | 1703.2 | win (violations) |
| pcbench-Omega2-mini-dock_Omega2 mini-dock | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 68.0 | | 8 | 573.5 | baseline |
| pcbench-Omega2-mini-dock_Omega2 mini-dock | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 12.1 | -55.9 | 1 | 548.5 | win (score) |
| pcbench-OpAmpPassXsistorBenchSupply_OpAmpPassXsistorBenchSupply | kicad | java-current | 0.00 | 3 | 3 | 977.6 | | unmeasured | 125.2 | | 1 | 1493.4 | baseline |
| pcbench-OpAmpPassXsistorBenchSupply_OpAmpPassXsistorBenchSupply | kicad | rs-main | 0.00 | 2 | 0 | 987.6 | +9.9 | | 33.6 | -91.7 | 1 | 1609.1 | win (unrouted) |
| pcbench-Open-Source-Power-Supply_PowerSupply_PCB | kicad | java-current | 0.00 | 0 | 3 | 993.2 | | unmeasured | 62.8 | | 7 | 1053.6 | baseline |
| pcbench-Open-Source-Power-Supply_PowerSupply_PCB | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +6.8 | | 4.6 | -58.2 | 2 | 1067.1 | win (clean_pass_rate) |
| pcbench-Open-Source-Power-Supply_PowerSupply_PCB_backup | kicad | java-current | 0.00 | 0 | 3 | 993.3 | | unmeasured | 71.8 | | 8 | 1072.3 | baseline |
| pcbench-Open-Source-Power-Supply_PowerSupply_PCB_backup | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +6.7 | | 6.1 | -65.7 | 2 | 1081.1 | win (clean_pass_rate) |
| pcbench-OpenHardwareExG_ActiveElectrode_OpenHardwareExG_ActiveElectrode | kicad | java-current | 0.00 | 3 | 3 | 861.5 | | unmeasured | 103.6 | | 0 | 123.1 | baseline |
| pcbench-OpenHardwareExG_ActiveElectrode_OpenHardwareExG_ActiveElectrode | kicad | rs-main | 0.00 | 3 | 0 | 884.6 | +23.1 | | 35.6 | -68.0 | 0 | 117.7 | win (violations) |
| pcbench-OpenHardwareExG_Shield_OpenHardwareExG_Shield | kicad | java-current | 0.00 | 45 | 119 | 819.9 | | unmeasured | 361.9 | | 73 | 2533.9 | baseline |
| pcbench-OpenHardwareExG_Shield_OpenHardwareExG_Shield | kicad | rs-main | 0.00 | 1 | 42 | 975.4 | +155.5 | | 190.0 | -171.9 | 101 | 3168.2 | win (unrouted) |
| pcbench-OpenHardwareExG_Shield_OpenHardwareExG_Shield_Test_Board | kicad | java-current | 0.00 | 3 | 64 | 933.6 | | unmeasured | 353.2 | | 73 | 3478.4 | baseline |
| pcbench-OpenHardwareExG_Shield_OpenHardwareExG_Shield_Test_Board | kicad | rs-main | 0.00 | 0 | 2 | 998.3 | +64.7 | | 48.7 | -304.5 | 60 | 3778.5 | win (unrouted) |
| pcbench-OpenHardwareExG_Shield_OpenHardwareExG_Shield_Test_Board_all_panelled | kicad | java-current | 0.00 | 3 | 66 | 931.9 | | unmeasured | 362.0 | | 73 | 3473.9 | baseline |
| pcbench-OpenHardwareExG_Shield_OpenHardwareExG_Shield_Test_Board_all_panelled | kicad | rs-main | 0.00 | 0 | 2 | 998.3 | +66.4 | | 72.3 | -289.7 | 60 | 3778.5 | win (unrouted) |
| pcbench-OpenVNAVI_driver unit | kicad | java-current | 0.00 | 4 | 23 | 972.0 | | unmeasured | 354.2 | | 100 | 2918.9 | baseline |
| pcbench-OpenVNAVI_driver unit | kicad | rs-main | 0.00 | 2 | 0 | 993.5 | +21.5 | | 78.9 | -275.3 | 88 | 2888.5 | win (unrouted) |
| pcbench-OpenVNAVI_motor unit | kicad | java-current | 0.00 | 1 | 0 | 900.0 | | unmeasured | 27.3 | | 1 | 48.8 | baseline |
| pcbench-OpenVNAVI_motor unit | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +100.0 | | 0.2 | -27.1 | 1 | 59.5 | win (clean_pass_rate) |
| pcbench-Own-Mailbox-Hardware_eth | kicad | java-current | 0.00 | 35 | 0 | 880.9 | | unmeasured | 392.6 | | 167 | 1774.6 | baseline |
| pcbench-Own-Mailbox-Hardware_eth | kicad | rs-main | 0.00 | 2 | 0 | 993.2 | +112.2 | | 256.6 | -136.1 | 168 | 2332.2 | win (unrouted) |
| pcbench-Own-Mailbox-Hardware_mailbox | kicad | java-current | 0.00 | 42 | 0 | 856.6 | | unmeasured | 392.3 | | 161 | 1704.5 | baseline |
| pcbench-Own-Mailbox-Hardware_mailbox | kicad | rs-main | 0.00 | 1 | 0 | 996.6 | +139.9 | | 281.7 | -110.6 | 159 | 2298.1 | win (unrouted) |
| pcbench-PCB_constant_current_ac_hv | kicad | java-current | 0.00 | 0 | 5 | 888.9 | | unmeasured | 13.5 | | 1 | 137.0 | baseline |
| pcbench-PCB_constant_current_ac_hv | kicad | rs-main | 0.00 | 0 | 8 | 822.2 | -66.7 | | 0.1 | -13.4 | 0 | 138.3 | loss (violations) |
| pcbench-PCB_serie_led_strip | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 12.0 | | 0 | 39.1 | baseline |
| pcbench-PCB_serie_led_strip | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.0 | -12.0 | 0 | 39.1 | win (cpu_s) |
| pcbench-PCB_small_halogen_replacement | kicad | java-current | 0.00 | 6 | 0 | 884.6 | | unmeasured | 113.8 | | 9 | 291.3 | baseline |
| pcbench-PCB_small_halogen_replacement | kicad | rs-main | 0.00 | 6 | 0 | 884.6 | +0.0 | | 57.2 | -56.6 | 1 | 295.9 | win (score) |
| pcbench-PGA2311_pga2311 | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 130.4 | | 8 | 561.4 | baseline |
| pcbench-PGA2311_pga2311 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 4.7 | -125.7 | 6 | 538.9 | win (score) |
| pcbench-POV_POV | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 43.2 | | 0 | 317.9 | baseline |
| pcbench-POV_POV | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 1.8 | -41.4 | 0 | 308.3 | win (score) |
| pcbench-PWRmeter_PWMeter | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 52.9 | | 5 | 547.0 | baseline |
| pcbench-PWRmeter_PWMeter | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 4.1 | -48.7 | 0 | 520.9 | win (score) |
| pcbench-Paperino_HW_Paperino_shield | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 57.0 | | 0 | 573.4 | baseline |
| pcbench-Paperino_HW_Paperino_shield | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 1.6 | -55.4 | 0 | 570.6 | win (score) |
| pcbench-Paperino_HW_paperino_breakout | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 25.5 | | 0 | 163.2 | baseline |
| pcbench-Paperino_HW_paperino_breakout | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.2 | -25.3 | 0 | 163.2 | win (cpu_s) |
| pcbench-Phased-Array-Microphone-using-FPGA_SateliteMicrophone | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 35.4 | | 8 | 227.3 | baseline |
| pcbench-Phased-Array-Microphone-using-FPGA_SateliteMicrophone | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 2.7 | -32.6 | 5 | 233.5 | win (score) |
| pcbench-PixyWirelessShield_Shield PIXY | kicad | java-current | 0.00 | 0 | 2 | 920.0 | | unmeasured | 15.9 | | 1 | 146.2 | baseline |
| pcbench-PixyWirelessShield_Shield PIXY | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +80.0 | | 0.1 | -15.9 | 0 | 152.0 | win (clean_pass_rate) |
| pcbench-PmodHDMIIn_PmodHDMIIn | kicad | java-current | 0.00 | 1 | 0 | 991.4 | | unmeasured | 139.4 | | 37 | 816.5 | baseline |
| pcbench-PmodHDMIIn_PmodHDMIIn | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +8.6 | | 44.6 | -94.8 | 12 | 867.3 | win (clean_pass_rate) |
| pcbench-PocketBone_pocketbone-kicad | kicad | java-current | 0.00 | 48 | 9 | 752.2 | | unmeasured | 339.8 | | 10 | 805.3 | baseline |
| pcbench-PocketBone_pocketbone-kicad | kicad | rs-main | 0.00 | 45 | 0 | 776.1 | +23.9 | | 195.1 | -144.7 | 10 | 828.3 | win (unrouted) |
| pcbench-Practicas-Curso-Kicad_Ejercicio_2 | kicad | java-current | 0.00 | 0 | 37 | 831.8 | | unmeasured | 66.3 | | 14 | 486.0 | baseline |
| pcbench-Practicas-Curso-Kicad_Ejercicio_2 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +168.2 | | 10.5 | -55.8 | 9 | 502.7 | win (clean_pass_rate) |
| pcbench-Practicas-Curso-Kicad_Salguero_Federico2 | kicad | java-current | 0.00 | 0 | 30 | 860.5 | | unmeasured | 79.6 | | 14 | 523.6 | baseline |
| pcbench-Practicas-Curso-Kicad_Salguero_Federico2 | kicad | rs-main | 0.00 | 0 | 19 | 911.6 | +51.2 | | 17.2 | -62.4 | 8 | 609.3 | win (violations) |
| pcbench-Prototyping_Workshop_Prototyping_PCB | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 87.3 | | 3 | 716.3 | baseline |
| pcbench-Prototyping_Workshop_Prototyping_PCB | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 4.1 | -83.2 | 1 | 666.3 | win (score) |
| pcbench-PsuFanController_FanController | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 21.2 | | 0 | 159.6 | baseline |
| pcbench-PsuFanController_FanController | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.4 | -20.8 | 0 | 158.3 | win (score) |
| pcbench-QRPCard_QRPCard | kicad | java-current | 0.00 | 0 | 38 | 917.4 | | unmeasured | 102.1 | | 23 | 862.1 | baseline |
| pcbench-QRPCard_QRPCard | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +82.6 | | 35.5 | -66.6 | 9 | 918.8 | win (clean_pass_rate) |
| pcbench-R1002_R1002 | kicad | java-current | 0.00 | 1 | 31 | 916.3 | | unmeasured | 151.6 | | 15 | 544.5 | baseline |
| pcbench-R1002_R1002 | kicad | rs-main | 0.00 | 3 | 0 | 965.1 | +48.8 | | 12.4 | -139.1 | 17 | 529.8 | loss (unrouted) |
| pcbench-RC2014_RC2014 IDE | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 160.6 | | 1 | 1461.8 | baseline |
| pcbench-RC2014_RC2014 IDE | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 10.5 | -150.1 | 4 | 1429.3 | loss (score) |
| pcbench-RC2014_RC2014 RAM | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 186.7 | | 11 | 1973.6 | baseline |
| pcbench-RC2014_RC2014 RAM | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 10.6 | -176.1 | 9 | 1985.3 | win (score) |
| pcbench-RC2014_RC2014 Tandy Sound Card | kicad | java-current | 0.00 | 2 | 0 | 984.0 | | unmeasured | 201.9 | | 13 | 2261.1 | baseline |
| pcbench-RC2014_RC2014 Tandy Sound Card | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +16.0 | | 39.4 | -162.6 | 17 | 2529.9 | win (clean_pass_rate) |
| pcbench-RFM69HCW_ATSHA204A_Breakout_RFM69HCW_ATSHA204A_Breakout | kicad | java-current | 0.00 | 1 | 17 | 866.7 | | unmeasured | 61.0 | | 3 | 182.5 | baseline |
| pcbench-RFM69HCW_ATSHA204A_Breakout_RFM69HCW_ATSHA204A_Breakout | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +133.3 | | 2.7 | -58.3 | 1 | 196.0 | win (clean_pass_rate) |
| pcbench-RGBMatrixPanelCPLD-PhotonBackpack_RGBMatrixPanel_CPLD_negative | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 344.2 | | 17 | 1232.9 | baseline |
| pcbench-RGBMatrixPanelCPLD-PhotonBackpack_RGBMatrixPanel_CPLD_negative | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 33.8 | -310.4 | 17 | 1198.6 | win (score) |
| pcbench-RGBMatrixPanelCPLD-PhotonBackpack_RGBMatrixPanel_CPLD_positive | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 342.1 | | 17 | 1233.2 | baseline |
| pcbench-RGBMatrixPanelCPLD-PhotonBackpack_RGBMatrixPanel_CPLD_positive | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 49.4 | -292.7 | 17 | 1198.6 | win (score) |
| pcbench-RPi-PWM-Fan-interface_RPi PWM Fan interface | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 12.9 | | 0 | 64.7 | baseline |
| pcbench-RPi-PWM-Fan-interface_RPi PWM Fan interface | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.1 | -12.8 | 0 | 63.8 | win (score) |
| pcbench-RX5808_diversityModule | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 323.4 | | 11 | 624.8 | baseline |
| pcbench-RX5808_diversityModule | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 14.8 | -308.6 | 13 | 548.6 | loss (score) |
| pcbench-RX5808_rx5808_4button | kicad | java-current | 0.00 | 1 | 0 | 992.2 | | unmeasured | 243.3 | | 54 | 1073.1 | baseline |
| pcbench-RX5808_rx5808_4button | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +7.8 | | 54.0 | -189.3 | 45 | 1011.8 | win (clean_pass_rate) |
| pcbench-Raspberry-Pi-Soft-Power-Controller_Switching Supply TPS563208 MCI | kicad | java-current | 0.00 | 0 | 6 | 929.4 | | unmeasured | 19.3 | | 7 | 151.1 | baseline |
| pcbench-Raspberry-Pi-Soft-Power-Controller_Switching Supply TPS563208 MCI | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +70.6 | | 1.5 | -17.8 | 0 | 174.7 | win (clean_pass_rate) |
| pcbench-Raspberry-Pi-Soft-Power-Controller_Zero Current Soft Power | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 34.1 | | 5 | 339.5 | baseline |
| pcbench-Raspberry-Pi-Soft-Power-Controller_Zero Current Soft Power | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 3.7 | -30.4 | 0 | 322.9 | win (score) |
| pcbench-ReST32_ReST RRD-FGC-Adapter | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 96.4 | | 17 | 755.6 | baseline |
| pcbench-ReST32_ReST RRD-FGC-Adapter | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 23.6 | -72.9 | 11 | 740.9 | win (score) |
| pcbench-ReST32_ReST SD-Module | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 71.8 | | 14 | 467.9 | baseline |
| pcbench-ReST32_ReST SD-Module | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 17.4 | -54.4 | 7 | 506.2 | win (score) |
| pcbench-Retro1DecodingModules_AddressDecoderModule | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 56.8 | | 1 | 536.7 | baseline |
| pcbench-Retro1DecodingModules_AddressDecoderModule | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 2.2 | -54.6 | 1 | 529.9 | win (score) |
| pcbench-RoBoC_CameraAdaptor | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 47.4 | | 5 | 696.2 | baseline |
| pcbench-RoBoC_CameraAdaptor | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 2.8 | -44.7 | 2 | 766.7 | win (score) |
| pcbench-RoBoC_RoboticsMKII | kicad | java-current | 0.00 | 43 | 47 | 779.8 | | unmeasured | 370.5 | | 100 | 2010.2 | baseline |
| pcbench-RoBoC_RoboticsMKII | kicad | rs-main | 0.00 | 0 | 14 | 988.2 | +208.4 | | 218.0 | -152.5 | 79 | 3462.4 | win (unrouted) |
| pcbench-S1G-Mod_JST_Adapter | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 19.1 | | 0 | 59.5 | baseline |
| pcbench-S1G-Mod_JST_Adapter | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 0.1 | -19.0 | 0 | 59.5 | loss (score) |
| pcbench-S4A-Mini-board_s4a-mini-board | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 206.2 | | 4 | 1071.3 | baseline |
| pcbench-S4A-Mini-board_s4a-mini-board | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 4.4 | -201.8 | 5 | 1127.6 | loss (score) |
| pcbench-SMDBreakouts_smd_breakout | kicad | java-current | 0.00 | 0 | 72 | 775.0 | | unmeasured | 49.2 | | 12 | 242.1 | baseline |
| pcbench-SMDBreakouts_smd_breakout | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +225.0 | | 1.5 | -47.7 | 0 | 293.2 | win (clean_pass_rate) |
| pcbench-SMDBreakouts_smd_breakout_quad | kicad | java-current | 0.00 | 0 | 72 | 775.0 | | unmeasured | 53.9 | | 12 | 242.1 | baseline |
| pcbench-SMDBreakouts_smd_breakout_quad | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +225.0 | | 1.0 | -52.9 | 0 | 293.2 | win (clean_pass_rate) |
| pcbench-SNAP-Badge_SNAP_badge | kicad | java-current | 0.00 | 61 | 1 | 822.6 | | unmeasured | 373.8 | | 150 | 2930.7 | baseline |
| pcbench-SNAP-Badge_SNAP_badge | kicad | rs-main | 0.00 | 25 | 0 | 927.5 | +104.9 | | 208.8 | -165.0 | 130 | 4211.9 | win (unrouted) |
| pcbench-STM32F303_LQFP48_STM32_LQFP48 | kicad | java-current | 0.00 | 0 | 2 | 995.1 | | unmeasured | 279.3 | | 38 | 1024.0 | baseline |
| pcbench-STM32F303_LQFP48_STM32_LQFP48 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +4.9 | | 94.1 | -185.1 | 24 | 1013.7 | win (clean_pass_rate) |
| pcbench-STM32F373_LQFP48_STM32_LQFP48 | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 370.6 | | 39 | 1050.4 | baseline |
| pcbench-STM32F373_LQFP48_STM32_LQFP48 | kicad | rs-main | 0.00 | 46 | 0 | 0.0 | -1000.0 | | 298.9 | -71.6 | 0 | 0.0 | loss (clean_pass_rate) |
| pcbench-Shift-in-32-HC165_shift-in | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 158.6 | | 3 | 1574.6 | baseline |
| pcbench-Shift-in-32-HC165_shift-in | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 37.2 | -121.4 | 0 | 1582.5 | win (score) |
| pcbench-Shift-out-32-HC595_Shift-out | kicad | java-current | 0.00 | 1 | 0 | 994.4 | | unmeasured | 200.0 | | 54 | 1489.9 | baseline |
| pcbench-Shift-out-32-HC595_Shift-out | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +5.6 | | 109.4 | -90.6 | 34 | 1493.3 | win (clean_pass_rate) |
| pcbench-SimpleCPLD_SimpleCPLD | kicad | java-current | 0.00 | 2 | 13 | 936.1 | | unmeasured | 230.9 | | 41 | 738.8 | baseline |
| pcbench-SimpleCPLD_SimpleCPLD | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +63.9 | | 79.8 | -151.1 | 31 | 796.2 | win (clean_pass_rate) |
| pcbench-SmartLaserCO2-PCB_LaserPointer | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 16.5 | | 0 | 49.8 | baseline |
| pcbench-SmartLaserCO2-PCB_LaserPointer | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.0 | -16.5 | 0 | 49.8 | win (cpu_s) |
| pcbench-SmartLaserCO2-PCB_OptAdjust | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 16.1 | | 0 | 79.3 | baseline |
| pcbench-SmartLaserCO2-PCB_OptAdjust | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.0 | -16.1 | 0 | 79.3 | win (cpu_s) |
| pcbench-SmartLaserCO2-PCB_WaterCool | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 17.7 | | 0 | 112.7 | baseline |
| pcbench-SmartLaserCO2-PCB_WaterCool | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.1 | -17.6 | 0 | 107.3 | win (score) |
| pcbench-Solare-BQ24210_Solare-BQ24210 | kicad | java-current | 0.00 | 0 | 8 | 942.9 | | unmeasured | 45.6 | | 1 | 121.2 | baseline |
| pcbench-Solare-BQ24210_Solare-BQ24210 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +57.1 | | 1.4 | -44.2 | 0 | 138.0 | win (clean_pass_rate) |
| pcbench-SparkSwitch_SparkProtectionSwitch | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 34.6 | | 4 | 182.0 | baseline |
| pcbench-SparkSwitch_SparkProtectionSwitch | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.9 | -33.7 | 0 | 204.4 | win (score) |
| pcbench-Starburst-One_alpha | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 53.0 | | 2 | 98.8 | baseline |
| pcbench-Starburst-One_alpha | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.7 | -52.3 | 2 | 96.3 | win (score) |
| pcbench-Starling_Starling_V1 | kicad | java-current | 0.00 | 1 | 19 | 939.2 | | unmeasured | 291.7 | | 39 | 1347.7 | baseline |
| pcbench-Starling_Starling_V1 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +60.8 | | 97.7 | -194.0 | 41 | 1328.0 | win (clean_pass_rate) |
| pcbench-Starling__autosave-Starling WiPSU ver_0.1 | kicad | java-current | 0.00 | 2 | 0 | 959.2 | | unmeasured | 100.0 | | 6 | 377.8 | baseline |
| pcbench-Starling__autosave-Starling WiPSU ver_0.1 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +40.8 | | 13.7 | -86.3 | 1 | 416.0 | win (clean_pass_rate) |
| pcbench-SynthDrumTrigger_Synth Drum Trigger | kicad | java-current | 0.00 | 1 | 0 | 980.4 | | unmeasured | 53.0 | | 0 | 434.2 | baseline |
| pcbench-SynthDrumTrigger_Synth Drum Trigger | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +19.6 | | 2.3 | -50.7 | 0 | 437.5 | win (clean_pass_rate) |
| pcbench-TB6600StepperDriver_DEW_TB6600-V1 | kicad | java-current | 0.00 | 24 | 0 | 853.7 | | unmeasured | 364.0 | | 3 | 1457.3 | baseline |
| pcbench-TB6600StepperDriver_DEW_TB6600-V1 | kicad | rs-main | 0.00 | 25 | 0 | 847.6 | -6.1 | | 108.2 | -255.8 | 1 | 1506.6 | loss (unrouted) |
| pcbench-TLPHnodeV2_TLPHnodeV2 | kicad | java-current | 0.00 | 1 | 19 | 950.0 | | unmeasured | 169.0 | | 18 | 433.2 | baseline |
| pcbench-TLPHnodeV2_TLPHnodeV2 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +50.0 | | 40.4 | -128.6 | 8 | 461.5 | win (clean_pass_rate) |
| pcbench-TMC261-stepstick_TMC261-stepstick-v1.1 | kicad | java-current | 0.00 | 17 | 54 | 885.1 | | unmeasured | 339.9 | | 64 | 2572.4 | baseline |
| pcbench-TMC261-stepstick_TMC261-stepstick-v1.1 | kicad | rs-main | 0.00 | 13 | 0 | 946.3 | +61.2 | | 103.7 | -236.2 | 47 | 3050.1 | win (unrouted) |
| pcbench-TX5823_TX5823 | kicad | java-current | 0.00 | 12 | 2 | 956.0 | | unmeasured | 344.9 | | 65 | 1429.2 | baseline |
| pcbench-TX5823_TX5823 | kicad | rs-main | 0.00 | 6 | 2 | 977.3 | +21.3 | | 156.7 | -188.2 | 58 | 1519.6 | win (unrouted) |
| pcbench-Teensy-3.5-Breakout-Boaard_PropShield_Uno | kicad | java-current | 0.00 | 1 | 0 | 969.7 | | unmeasured | 87.8 | | 8 | 1072.3 | baseline |
| pcbench-Teensy-3.5-Breakout-Boaard_PropShield_Uno | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +30.3 | | 11.5 | -76.3 | 5 | 1137.3 | win (clean_pass_rate) |
| pcbench-Teensy-3.5-Breakout-Boaard_TeensyMegaIoShield | kicad | java-current | 0.00 | 13 | 0 | 937.2 | | unmeasured | 368.9 | | 23 | 2812.7 | baseline |
| pcbench-Teensy-3.5-Breakout-Boaard_TeensyMegaIoShield | kicad | rs-main | 0.00 | 1 | 0 | 995.2 | +58.0 | | 209.7 | -159.3 | 49 | 3892.8 | win (unrouted) |
| pcbench-Teensy-3.5-Breakout-Boaard_Test | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 39.3 | | 0 | 843.3 | baseline |
| pcbench-Teensy-3.5-Breakout-Boaard_Test | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 1.6 | -37.7 | 0 | 843.3 | win (cpu_s) |
| pcbench-Teensy-Hats_Teensy-7-Segment-Hat | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 56.9 | | 2 | 181.3 | baseline |
| pcbench-Teensy-Hats_Teensy-7-Segment-Hat | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 0.9 | -56.0 | 4 | 176.0 | loss (score) |
| pcbench-Teensy-Hats_Teensy-LCD-LiDAR-Hat | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 63.9 | | 8 | 756.9 | baseline |
| pcbench-Teensy-Hats_Teensy-LCD-LiDAR-Hat | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 4.1 | -59.8 | 2 | 802.6 | win (score) |
| pcbench-TeensyProtoboard_TeensyProtoboard | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 56.8 | | 2 | 537.8 | baseline |
| pcbench-TeensyProtoboard_TeensyProtoboard | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 8.7 | -48.2 | 2 | 535.6 | win (score) |
| pcbench-ThinkerShield_ThinkerShield | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 187.7 | | 10 | 1016.5 | baseline |
| pcbench-ThinkerShield_ThinkerShield | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 9.0 | -178.7 | 7 | 984.9 | win (score) |
| pcbench-TinyTracker_ub-minimal | kicad | java-current | 0.00 | 4 | 70 | 888.2 | | unmeasured | 312.4 | | 36 | 602.4 | baseline |
| pcbench-TinyTracker_ub-minimal | kicad | rs-main | 0.00 | 3 | 0 | 981.4 | +93.2 | | 62.0 | -250.4 | 35 | 620.7 | win (unrouted) |
| pcbench-ToslinkCNC_Toslink PlanetCNC ECO shield | kicad | java-current | 0.00 | 1 | 0 | 961.5 | | unmeasured | 75.1 | | 2 | 971.1 | baseline |
| pcbench-ToslinkCNC_Toslink PlanetCNC ECO shield | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +38.5 | | 2.9 | -72.1 | 3 | 1045.3 | win (clean_pass_rate) |
| pcbench-ToslinkCNC__autosave-ToslinkCNC_OneAxis | kicad | java-current | 0.00 | 8 | 19 | 904.8 | | unmeasured | 253.1 | | 32 | 1049.9 | baseline |
| pcbench-ToslinkCNC__autosave-ToslinkCNC_OneAxis | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +95.2 | | 161.0 | -92.1 | 37 | 1220.7 | win (clean_pass_rate) |
| pcbench-ToslinkCNC_toslink_arduino_shield | kicad | java-current | 0.00 | 3 | 0 | 961.5 | | unmeasured | 166.2 | | 24 | 1671.1 | baseline |
| pcbench-ToslinkCNC_toslink_arduino_shield | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +38.5 | | 62.1 | -104.1 | 18 | 1937.5 | win (clean_pass_rate) |
| pcbench-TripleDelay2399_TripleDelay2399 | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 122.5 | | 1 | 2163.5 | baseline |
| pcbench-TripleDelay2399_TripleDelay2399 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 16.1 | -106.4 | 0 | 2152.2 | win (score) |
| pcbench-ULPI-Pmod_ULPI-Pmod | kicad | java-current | 0.00 | 2 | 16 | 920.0 | | unmeasured | 220.6 | | 21 | 559.0 | baseline |
| pcbench-ULPI-Pmod_ULPI-Pmod | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +80.0 | | 79.2 | -141.4 | 13 | 578.6 | win (clean_pass_rate) |
| pcbench-UProgrammer-Hardware_Programmer | kicad | java-current | 0.00 | 18 | 21 | 882.5 | | unmeasured | 387.6 | | 54 | 1888.4 | baseline |
| pcbench-UProgrammer-Hardware_Programmer | kicad | rs-main | 0.00 | 21 | 0 | 888.9 | +6.3 | | 129.8 | -257.8 | 55 | 1693.3 | loss (unrouted) |
| pcbench-UltrasonicSystem_Schematic | kicad | java-current | 0.00 | 0 | 16 | 981.8 | | unmeasured | 202.8 | | 35 | 3393.5 | baseline |
| pcbench-UltrasonicSystem_Schematic | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +18.2 | | 51.6 | -151.2 | 11 | 3451.7 | win (clean_pass_rate) |
| pcbench-UniversalBoard4Nucleo_Nucleo_Universal_Board | kicad | java-current | 0.00 | 1 | 0 | 991.5 | | unmeasured | 98.1 | | 0 | 1241.7 | baseline |
| pcbench-UniversalBoard4Nucleo_Nucleo_Universal_Board | kicad | rs-main | 0.00 | 1 | 0 | 991.5 | -0.0 | | 32.8 | -65.3 | 0 | 1259.8 | loss (score) |
| pcbench-Usb-Serial-Breakout-Cp2102_cp2102 | kicad | java-current | 0.00 | 3 | 10 | 791.7 | | unmeasured | 112.8 | | 10 | 149.1 | baseline |
| pcbench-Usb-Serial-Breakout-Cp2102_cp2102 | kicad | rs-main | 0.00 | 3 | 0 | 875.0 | +83.3 | | 22.5 | -90.2 | 8 | 135.4 | win (violations) |
| pcbench-VC4000MultiROM_MultiRomCard | kicad | java-current | 0.00 | 3 | 10 | 978.1 | | unmeasured | 344.4 | | 31 | 6344.1 | baseline |
| pcbench-VC4000MultiROM_MultiRomCard | kicad | rs-main | 0.00 | 6 | 6 | 968.4 | -9.6 | | 132.1 | -212.3 | 33 | 6223.6 | loss (unrouted) |
| pcbench-VM-sensor-PT1000_vm-sensor-pt100 | kicad | java-current | 0.00 | 2 | 0 | 960.0 | | unmeasured | 104.5 | | 9 | 372.7 | baseline |
| pcbench-VM-sensor-PT1000_vm-sensor-pt100 | kicad | rs-main | 0.00 | 1 | 0 | 980.0 | +20.0 | | 16.5 | -88.0 | 6 | 390.2 | win (unrouted) |
| pcbench-WHCS-Base-Station_base-station | kicad | java-current | 0.00 | 0 | 15 | 976.6 | | unmeasured | 197.7 | | 42 | 2783.9 | baseline |
| pcbench-WHCS-Base-Station_base-station | kicad | rs-main | 0.00 | 0 | 1 | 998.4 | +21.9 | | 88.7 | -109.0 | 37 | 2840.0 | win (violations) |
| pcbench-WS2811LEDMatrix_matrixcontrol | kicad | java-current | 0.00 | 0 | 27 | 952.2 | | unmeasured | 276.1 | | 43 | 1433.7 | baseline |
| pcbench-WS2811LEDMatrix_matrixcontrol | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +47.8 | | 86.3 | -189.8 | 34 | 1463.1 | win (clean_pass_rate) |
| pcbench-WeatherSpot_vreg_pressure | kicad | java-current | 0.00 | 0 | 4 | 933.3 | | unmeasured | 15.1 | | 2 | 59.3 | baseline |
| pcbench-WeatherSpot_vreg_pressure | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +66.7 | | 0.2 | -14.9 | 0 | 59.9 | win (clean_pass_rate) |
| pcbench-WordClock_v2.0_WordClock_v2 | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 54.2 | | 30 | 546.1 | baseline |
| pcbench-WordClock_v2.0_WordClock_v2 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 11.1 | -43.2 | 7 | 579.4 | win (score) |
| pcbench-Youyue-858D-plus-MCU-adapter_youyue-858d-plus-mcu-adapter | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 40.6 | | 4 | 451.7 | baseline |
| pcbench-Youyue-858D-plus-MCU-adapter_youyue-858d-plus-mcu-adapter | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 4.3 | -36.3 | 4 | 410.7 | win (score) |
| pcbench-abus-cfa1000-display-grabber_acs-display-grabber | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 216.8 | | 112 | 3373.1 | baseline |
| pcbench-abus-cfa1000-display-grabber_acs-display-grabber | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 100.3 | -116.5 | 70 | 3474.3 | win (score) |
| pcbench-airqualitystation_hardware | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 78.9 | | 24 | 657.5 | baseline |
| pcbench-airqualitystation_hardware | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 26.3 | -52.6 | 16 | 665.8 | win (score) |
| pcbench-akuhei_akuhei | kicad | java-current | 0.00 | 2 | 0 | 944.4 | | unmeasured | 108.8 | | 12 | 273.4 | baseline |
| pcbench-akuhei_akuhei | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +55.6 | | 14.2 | -94.6 | 7 | 314.3 | win (clean_pass_rate) |
| pcbench-analog_esr_meter_esr_meter_rev_a | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 52.6 | | 1 | 813.9 | baseline |
| pcbench-analog_esr_meter_esr_meter_rev_a | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 6.4 | -46.2 | 1 | 786.5 | win (score) |
| pcbench-anima_MotorDrive | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 154.6 | | 30 | 1640.4 | baseline |
| pcbench-anima_MotorDrive | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 61.0 | -93.7 | 22 | 1543.7 | win (score) |
| pcbench-antdroid-board_antdroid-board | kicad | java-current | 0.00 | 22 | 0 | 841.7 | | unmeasured | 197.1 | | 0 | 1359.9 | baseline |
| pcbench-antdroid-board_antdroid-board | kicad | rs-main | 0.00 | 18 | 0 | 870.5 | +28.8 | | 84.8 | -112.4 | 0 | 1494.4 | win (unrouted) |
| pcbench-apa102lantern_apa102-lantern-side | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 43.2 | | 18 | 568.2 | baseline |
| pcbench-apa102lantern_apa102-lantern-side | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 9.5 | -33.7 | 0 | 681.4 | win (score) |
| pcbench-arduino-led-driver_arduino-led-driver | kicad | java-current | 0.00 | 10 | 114 | 882.0 | | unmeasured | 352.9 | | 94 | 2025.9 | baseline |
| pcbench-arduino-led-driver_arduino-led-driver | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +118.0 | | 271.2 | -81.6 | 75 | 2767.4 | win (clean_pass_rate) |
| pcbench-arduino_arduino leds | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 52.0 | | 1 | 753.1 | baseline |
| pcbench-arduino_arduino leds | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 3.7 | -48.3 | 1 | 730.2 | win (score) |
| pcbench-atmegax8-protoboard_atmegax8-protoboard | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 53.7 | | 0 | 328.7 | baseline |
| pcbench-atmegax8-protoboard_atmegax8-protoboard | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.8 | -52.9 | 0 | 327.4 | win (score) |
| pcbench-atmel-programmer_atmel_programmer | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 52.4 | | 0 | 909.2 | baseline |
| pcbench-atmel-programmer_atmel_programmer | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 2.5 | -49.8 | 0 | 930.1 | loss (score) |
| pcbench-audio_relay_input_switch_relay_switch | kicad | java-current | 0.00 | 0 | 3 | 985.4 | | unmeasured | 96.3 | | 3 | 570.7 | baseline |
| pcbench-audio_relay_input_switch_relay_switch | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +14.6 | | 2.2 | -94.1 | 3 | 580.2 | win (clean_pass_rate) |
| pcbench-audprog_audprog_v2 | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 86.7 | | 25 | 748.9 | baseline |
| pcbench-audprog_audprog_v2 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 42.0 | -44.7 | 13 | 734.2 | win (score) |
| pcbench-autohat-board_inverted-usd-adapter | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 17.9 | | 1 | 235.1 | baseline |
| pcbench-autohat-board_inverted-usd-adapter | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.1 | -17.8 | 0 | 234.4 | win (score) |
| pcbench-autohat-board_usd-adapter | kicad | java-current | 0.00 | 2 | 3 | 675.0 | | unmeasured | 49.6 | | 4 | 200.8 | baseline |
| pcbench-autohat-board_usd-adapter | kicad | rs-main | 0.00 | 1 | 0 | 875.0 | +200.0 | | 5.0 | -44.6 | 4 | 235.2 | win (unrouted) |
| pcbench-avr-fuser-32_adapter | kicad | java-current | 0.00 | 8 | 8 | 939.2 | | unmeasured | 323.5 | | 4 | 4464.8 | baseline |
| pcbench-avr-fuser-32_adapter | kicad | rs-main | 0.00 | 1 | 0 | 993.7 | +54.4 | | 62.8 | -260.7 | 1 | 4959.9 | win (unrouted) |
| pcbench-avr_ledprojector_avr_ledprojection | kicad | java-current | 0.00 | 2 | 26 | 958.9 | | unmeasured | 202.3 | | 62 | 1146.5 | baseline |
| pcbench-avr_ledprojector_avr_ledprojection | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +41.1 | | 50.7 | -151.6 | 40 | 1103.3 | win (clean_pass_rate) |
| pcbench-avr_ledprojector_avr_ledprojection-0402 | kicad | java-current | 0.00 | 2 | 60 | 920.5 | | unmeasured | 329.5 | | 59 | 830.1 | baseline |
| pcbench-avr_ledprojector_avr_ledprojection-0402 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +79.5 | | 96.8 | -232.7 | 57 | 781.0 | win (clean_pass_rate) |
| pcbench-badge2016_Badge_init | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 243.7 | | 1 | 447.6 | baseline |
| pcbench-badge2016_Badge_init | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 5.0 | -238.7 | 3 | 441.3 | loss (score) |
| pcbench-balena-rover-wide-hat_resin-rover | kicad | java-current | 0.00 | 0 | 57 | 948.2 | | unmeasured | 286.1 | | 53 | 2000.7 | baseline |
| pcbench-balena-rover-wide-hat_resin-rover | kicad | rs-main | 0.00 | 0 | 4 | 996.4 | +48.2 | | 210.5 | -75.5 | 25 | 2065.9 | win (violations) |
| pcbench-basic_esp_board_basic_esp_board | kicad | java-current | 0.00 | 1 | 4 | 978.6 | | unmeasured | 141.2 | | 34 | 1171.3 | baseline |
| pcbench-basic_esp_board_basic_esp_board | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +21.4 | | 18.1 | -123.1 | 7 | 1305.9 | win (clean_pass_rate) |
| pcbench-beast-phat_beast-phat | kicad | java-current | 0.00 | 0 | 5 | 976.2 | | unmeasured | 60.9 | | 2 | 352.5 | baseline |
| pcbench-beast-phat_beast-phat | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +23.8 | | 4.3 | -56.5 | 0 | 357.7 | win (clean_pass_rate) |
| pcbench-bee-light-measurement-matrix_bee-light-measurement-matrix | kicad | java-current | 0.00 | 1 | 0 | 993.9 | | unmeasured | 174.3 | | 17 | 2252.5 | baseline |
| pcbench-bee-light-measurement-matrix_bee-light-measurement-matrix | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +6.1 | | 73.0 | -101.4 | 12 | 2303.4 | win (clean_pass_rate) |
| pcbench-beer-gauge_sensorboard | kicad | java-current | 0.00 | 0 | 1 | 994.9 | | unmeasured | 142.9 | | 2 | 209.9 | baseline |
| pcbench-beer-gauge_sensorboard | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +5.1 | | 4.0 | -138.8 | 0 | 213.1 | win (clean_pass_rate) |
| pcbench-beryl_rain_beryl_rain | kicad | java-current | 0.00 | 0 | 1 | 995.4 | | unmeasured | 84.3 | | 19 | 482.6 | baseline |
| pcbench-beryl_rain_beryl_rain | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +4.5 | | 10.3 | -74.0 | 12 | 474.8 | win (clean_pass_rate) |
| pcbench-bikedar_bikedar | kicad | java-current | 0.00 | 0 | 61 | 911.6 | | unmeasured | 178.1 | | 26 | 739.9 | baseline |
| pcbench-bikedar_bikedar | kicad | rs-main | 0.00 | 0 | 3 | 995.7 | +84.1 | | 60.4 | -117.7 | 14 | 740.4 | win (violations) |
| pcbench-blackmagic-isolated_mmp | kicad | java-current | 0.00 | 1 | 28 | 929.0 | | unmeasured | 267.5 | | 44 | 651.4 | baseline |
| pcbench-blackmagic-isolated_mmp | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +71.0 | | 69.1 | -198.4 | 36 | 636.0 | win (clean_pass_rate) |
| pcbench-bldc-gimbal-1d_gimbal-board | kicad | java-current | 0.00 | 0 | 38 | 909.5 | | unmeasured | 136.5 | | 33 | 893.5 | baseline |
| pcbench-bldc-gimbal-1d_gimbal-board | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +90.5 | | 50.2 | -86.3 | 22 | 904.9 | win (clean_pass_rate) |
| pcbench-blinky-badge_blinky | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 280.6 | | 7 | 505.6 | baseline |
| pcbench-blinky-badge_blinky | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 299.0 | +18.4 | 12 | 519.9 | loss (score) |
| pcbench-bms-8s50-ic_bms-8s50-ic | kicad | java-current | 0.00 | 168 | 41 | 608.4 | | unmeasured | 369.7 | | 82 | 1247.9 | baseline |
| pcbench-bms-8s50-ic_bms-8s50-ic | kicad | rs-main | 0.00 | 52 | 0 | 884.4 | +276.0 | | 300.2 | -69.5 | 106 | 2907.7 | win (unrouted) |
| pcbench-bmw-ibus-bluetooth_bmw_bt_cdcemu_analog | kicad | java-current | 0.00 | 1 | 2 | 982.9 | | unmeasured | 116.2 | | 8 | 393.7 | baseline |
| pcbench-bmw-ibus-bluetooth_bmw_bt_cdcemu_analog | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +17.1 | | 7.0 | -109.1 | 0 | 389.6 | win (clean_pass_rate) |
| pcbench-bmw-ibus-bluetooth_bmw_bt_cdcemu_digital | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 110.5 | | 15 | 929.7 | baseline |
| pcbench-bmw-ibus-bluetooth_bmw_bt_cdcemu_digital | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 43.5 | -67.0 | 10 | 942.4 | win (score) |
| pcbench-board_armjtag_pmod_compatible_armjtag-pmod | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 24.2 | | 0 | 275.9 | baseline |
| pcbench-board_armjtag_pmod_compatible_armjtag-pmod | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 0.4 | -23.8 | 0 | 276.1 | loss (score) |
| pcbench-boards_shift-register-demo-v2 | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 81.1 | | 0 | 1162.2 | baseline |
| pcbench-boards_shift-register-demo-v2 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 15.2 | -65.9 | 0 | 1145.7 | win (score) |
| pcbench-boatcontrol_CommonCathode60A | kicad | java-current | 0.00 | 98 | 0 | 228.3 | | unmeasured | 104.3 | | 3 | 923.7 | baseline |
| pcbench-boatcontrol_CommonCathode60A | kicad | rs-main | 0.00 | 95 | 0 | 252.0 | +23.6 | | 10.6 | -93.7 | 4 | 1059.9 | win (unrouted) |
| pcbench-boatcontrol_NonLatchingNO30A | kicad | java-current | 0.00 | 23 | 0 | 732.6 | | unmeasured | 278.0 | | 12 | 2036.3 | baseline |
| pcbench-boatcontrol_NonLatchingNO30A | kicad | rs-main | 0.00 | 20 | 0 | 767.4 | +34.9 | | 202.9 | -75.1 | 20 | 2359.1 | win (unrouted) |
| pcbench-bobc_LCD-panel-adapter-lvc | kicad | java-current | 0.00 | 2 | 20 | 853.7 | | unmeasured | 135.7 | | 14 | 771.6 | baseline |
| pcbench-bobc_LCD-panel-adapter-lvc | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +146.3 | | 27.0 | -108.7 | 11 | 840.1 | win (clean_pass_rate) |
| pcbench-bobc_MS-F100 | kicad | java-current | 0.00 | 14 | 37 | 808.9 | | unmeasured | 334.0 | | 21 | 902.4 | baseline |
| pcbench-bobc_MS-F100 | kicad | rs-main | 0.00 | 5 | 0 | 955.4 | +146.4 | | 76.0 | -257.9 | 20 | 1078.2 | win (unrouted) |
| pcbench-bobc_led_clock | kicad | java-current | 0.00 | 0 | 17 | 977.3 | | unmeasured | 207.1 | | 58 | 2442.1 | baseline |
| pcbench-bobc_led_clock | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +22.7 | | 148.2 | -58.9 | 47 | 2543.0 | win (clean_pass_rate) |
| pcbench-bobc_matrix_clock | kicad | java-current | 0.00 | 37 | 1 | 798.9 | | unmeasured | 355.0 | | 88 | 4261.0 | baseline |
| pcbench-bobc_matrix_clock | kicad | rs-main | 0.00 | 24 | 0 | 870.3 | +71.4 | | 108.0 | -247.0 | 85 | 4749.3 | win (unrouted) |
| pcbench-bpnode-bb_BPnode-BB | kicad | java-current | 0.00 | 1 | 3 | 961.9 | | unmeasured | 99.1 | | 15 | 408.3 | baseline |
| pcbench-bpnode-bb_BPnode-BB | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +38.1 | | 9.8 | -89.3 | 9 | 459.5 | win (clean_pass_rate) |
| pcbench-breakout-boards_50-to-100 | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 38.4 | | 4 | 113.0 | baseline |
| pcbench-breakout-boards_50-to-100 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 1.0 | -37.4 | 4 | 109.2 | win (score) |
| pcbench-breakout-boards_avr-isp-x2 | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 16.4 | | 0 | 42.2 | baseline |
| pcbench-breakout-boards_avr-isp-x2 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.0 | -16.4 | 0 | 42.2 | win (cpu_s) |
| pcbench-breakout-boards_esp8266-jtag | kicad | java-current | 0.00 | 0 | 50 | 791.7 | | unmeasured | 58.5 | | 11 | 408.6 | baseline |
| pcbench-breakout-boards_esp8266-jtag | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +208.3 | | 6.3 | -52.2 | 4 | 392.8 | win (clean_pass_rate) |
| pcbench-breakout-boards_swd-and-uart | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 37.5 | | 2 | 124.5 | baseline |
| pcbench-breakout-boards_swd-and-uart | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.8 | -36.7 | 2 | 120.5 | win (score) |
| pcbench-breakout-boards_swd-to-wires | kicad | java-current | 0.00 | 1 | 0 | 888.9 | | unmeasured | 45.1 | | 0 | 94.6 | baseline |
| pcbench-breakout-boards_swd-to-wires | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +111.1 | | 1.3 | -43.8 | 0 | 108.1 | win (clean_pass_rate) |
| pcbench-bristle_bot_light_follow_bristle_bot | kicad | java-current | 0.00 | 1 | 0 | 952.4 | | unmeasured | 33.8 | | 0 | 336.4 | baseline |
| pcbench-bristle_bot_light_follow_bristle_bot | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +47.6 | | 0.6 | -33.2 | 0 | 350.1 | win (clean_pass_rate) |
| pcbench-busblaster-to-swd_busblaster-to-swd | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 42.6 | | 4 | 238.4 | baseline |
| pcbench-busblaster-to-swd_busblaster-to-swd | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 1.2 | -41.4 | 0 | 236.7 | win (score) |
| pcbench-bypass_crossmix_bypass_crossmix | kicad | java-current | 0.00 | 12 | 14 | 878.7 | | unmeasured | 172.2 | | 24 | 1130.1 | baseline |
| pcbench-bypass_crossmix_bypass_crossmix | kicad | rs-main | 0.00 | 9 | 0 | 926.2 | +47.5 | | 21.0 | -151.2 | 25 | 1134.1 | win (unrouted) |
| pcbench-can_firewall_hardware_CAN_Firewall | kicad | java-current | 0.00 | 2 | 91 | 909.4 | | unmeasured | 347.4 | | 149 | 2242.6 | baseline |
| pcbench-can_firewall_hardware_CAN_Firewall | kicad | rs-main | 0.00 | 1 | 0 | 995.5 | +86.1 | | 140.1 | -207.3 | 121 | 2235.2 | win (unrouted) |
| pcbench-cdm324_backpack_cdm324 | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 123.7 | | 7 | 247.2 | baseline |
| pcbench-cdm324_backpack_cdm324 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 4.0 | -119.7 | 3 | 251.5 | win (score) |
| pcbench-ciurlys_ciurlys | kicad | java-current | 0.00 | 7 | 0 | 794.1 | | unmeasured | 126.5 | | 7 | 166.8 | baseline |
| pcbench-ciurlys_ciurlys | kicad | rs-main | 0.00 | 7 | 0 | 794.1 | -0.0 | | 60.0 | -66.5 | 14 | 181.3 | loss (score) |
| pcbench-clock_lcdb4 | kicad | java-current | 0.00 | 1 | 0 | 984.4 | | unmeasured | 128.0 | | 10 | 1249.7 | baseline |
| pcbench-clock_lcdb4 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +15.6 | | 27.3 | -100.6 | 7 | 1341.2 | win (clean_pass_rate) |
| pcbench-cnlohr_wiflier | kicad | java-current | 0.00 | 38 | 2 | 738.8 | | unmeasured | 359.9 | | 45 | 509.1 | baseline |
| pcbench-cnlohr_wiflier | kicad | rs-main | 0.00 | 41 | 0 | 721.1 | -17.7 | | 122.6 | -237.3 | 13 | 449.7 | loss (unrouted) |
| pcbench-cnlohr_wiflier_B | kicad | java-current | 0.00 | 35 | 0 | 778.5 | | unmeasured | 388.2 | | 32 | 505.4 | baseline |
| pcbench-cnlohr_wiflier_B | kicad | rs-main | 0.00 | 41 | 0 | 740.5 | -38.0 | | 88.8 | -299.4 | 26 | 487.9 | loss (unrouted) |
| pcbench-continuity-tester_continuity-tester | kicad | java-current | 0.00 | 1 | 11 | 893.3 | | unmeasured | 64.0 | | 6 | 207.2 | baseline |
| pcbench-continuity-tester_continuity-tester | kicad | rs-main | 0.00 | 1 | 4 | 940.0 | +46.7 | | 17.3 | -46.8 | 2 | 196.6 | win (violations) |
| pcbench-cookiecutter-xsproduct_{{cookiecutter.product_name}} | kicad | java-current | 0.00 | 1 | 0 | 937.5 | | unmeasured | 40.4 | | 0 | 168.5 | baseline |
| pcbench-cookiecutter-xsproduct_{{cookiecutter.product_name}} | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +62.5 | | 0.3 | -40.1 | 0 | 173.1 | win (clean_pass_rate) |
| pcbench-crossover-schiit-stack_xover4schiit | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 17.0 | | 0 | 170.8 | baseline |
| pcbench-crossover-schiit-stack_xover4schiit | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 0.2 | -16.8 | 0 | 174.0 | loss (score) |
| pcbench-custom_cpu--ALU_custom_cpu--ALU | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 98.8 | | 9 | 1864.3 | baseline |
| pcbench-custom_cpu--ALU_custom_cpu--ALU | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 15.6 | -83.2 | 12 | 1824.0 | loss (score) |
| pcbench-custom_cpu--register_custom_cpu--register | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 101.9 | | 0 | 1322.3 | baseline |
| pcbench-custom_cpu--register_custom_cpu--register | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 5.2 | -96.8 | 0 | 1331.1 | loss (score) |
| pcbench-data-manager_data-manager | kicad | java-current | 0.00 | 1 | 31 | 926.5 | | unmeasured | 185.8 | | 50 | 1190.4 | baseline |
| pcbench-data-manager_data-manager | kicad | rs-main | 0.00 | 30 | 0 | 693.9 | -232.7 | | 299.2 | +113.4 | 49 | 436.8 | loss (unrouted) |
| pcbench-decelerator4030_decelerator4030 | kicad | java-current | 0.00 | 499 | 205 | 464.8 | | unmeasured | 382.9 | | 518 | 2133.0 | baseline |
| pcbench-decelerator4030_decelerator4030 | kicad | rs-main | 0.00 | 488 | 5 | 515.4 | +50.5 | | 300.3 | -82.6 | 569 | 7007.6 | win (unrouted) |
| pcbench-denbit_basic | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 61.4 | | 0 | 622.8 | baseline |
| pcbench-denbit_basic | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 0.8 | -60.5 | 0 | 635.1 | loss (score) |
| pcbench-deskbot_breakout | kicad | java-current | 0.00 | 2 | 0 | 941.2 | | unmeasured | 57.2 | | 4 | 314.6 | baseline |
| pcbench-deskbot_breakout | kicad | rs-main | 0.00 | 1 | 0 | 970.6 | +29.4 | | 52.5 | -4.7 | 1 | 318.8 | win (unrouted) |
| pcbench-devttys0_IRis | kicad | java-current | 0.00 | 0 | 3 | 988.5 | | unmeasured | 55.4 | | 9 | 291.6 | baseline |
| pcbench-devttys0_IRis | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +11.5 | | 5.3 | -50.1 | 3 | 299.1 | win (clean_pass_rate) |
| pcbench-digital_clock_led_clock_3_and_4_digit | kicad | java-current | 0.00 | 0 | 3 | 997.6 | | unmeasured | 218.8 | | 31 | 4839.7 | baseline |
| pcbench-digital_clock_led_clock_3_and_4_digit | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +2.4 | | 27.2 | -191.5 | 24 | 4853.5 | win (clean_pass_rate) |
| pcbench-digital_clock_led_clock_v1 | kicad | java-current | 0.00 | 1 | 13 | 985.7 | | unmeasured | 196.8 | | 43 | 5372.9 | baseline |
| pcbench-digital_clock_led_clock_v1 | kicad | rs-main | 0.00 | 0 | 3 | 997.6 | +12.0 | | 61.6 | -135.2 | 20 | 5565.4 | win (unrouted) |
| pcbench-disco-dongle_DiscoDongle | kicad | java-current | 0.00 | 0 | 35 | 872.7 | | unmeasured | 144.1 | | 13 | 491.9 | baseline |
| pcbench-disco-dongle_DiscoDongle | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +127.3 | | 43.6 | -100.5 | 8 | 503.6 | win (clean_pass_rate) |
| pcbench-divergence_meter_dm_control | kicad | java-current | 0.00 | 8 | 59 | 924.7 | | unmeasured | 367.8 | | 76 | 1772.0 | baseline |
| pcbench-divergence_meter_dm_control | kicad | rs-main | 0.00 | 9 | 0 | 965.8 | +41.1 | | 117.6 | -250.2 | 67 | 1716.3 | loss (unrouted) |
| pcbench-domotics_base-board-arranged | kicad | java-current | 0.00 | 20 | 3 | 954.9 | | unmeasured | 389.5 | | 19 | 11209.3 | baseline |
| pcbench-domotics_base-board-arranged | kicad | rs-main | 0.00 | 0 | 13 | 994.3 | +39.4 | | 34.0 | -355.4 | 54 | 14628.5 | win (unrouted) |
| pcbench-dorkyboard_keyboard | kicad | java-current | 0.00 | 421 | 107 | 467.0 | | unmeasured | 327.0 | | 40 | 2875.3 | baseline |
| pcbench-dorkyboard_keyboard | kicad | rs-main | 0.00 | 3 | 16 | 992.5 | +525.5 | | 194.5 | -132.5 | 47 | 13557.2 | win (unrouted) |
| pcbench-drawduino_drawduino | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 25.4 | | 2 | 223.6 | baseline |
| pcbench-drawduino_drawduino | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.6 | -24.9 | 0 | 230.4 | win (score) |
| pcbench-dust_sensor_dust_sensor | kicad | java-current | 0.00 | 1 | 29 | 913.9 | | unmeasured | 201.8 | | 35 | 1319.6 | baseline |
| pcbench-dust_sensor_dust_sensor | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +86.1 | | 106.1 | -95.7 | 26 | 1432.8 | win (clean_pass_rate) |
| pcbench-dustbox_Dustbox | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 20.5 | | 0 | 261.8 | baseline |
| pcbench-dustbox_Dustbox | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.7 | -19.8 | 0 | 252.3 | win (score) |
| pcbench-eBUS-Adapter_Groeger | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 26.8 | | 0 | 249.8 | baseline |
| pcbench-eBUS-Adapter_Groeger | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 0.3 | -26.4 | 0 | 251.3 | loss (score) |
| pcbench-eeg_brainboard_batteryv0 | kicad | java-current | 0.00 | 5 | 18 | 945.2 | | unmeasured | 347.4 | | 31 | 1252.2 | baseline |
| pcbench-eeg_brainboard_batteryv0 | kicad | rs-main | 0.00 | 1 | 0 | 993.6 | +48.4 | | 64.8 | -282.6 | 27 | 1210.1 | win (unrouted) |
| pcbench-eink-adapter_eink | kicad | java-current | 0.00 | 35 | 0 | 771.2 | | unmeasured | 354.7 | | 22 | 1369.2 | baseline |
| pcbench-eink-adapter_eink | kicad | rs-main | 0.00 | 28 | 0 | 817.0 | +45.8 | | 95.7 | -259.0 | 26 | 1566.3 | win (unrouted) |
| pcbench-epapercard_epapercard | kicad | java-current | 0.00 | 6 | 2 | 943.4 | | unmeasured | 292.8 | | 69 | 1780.3 | baseline |
| pcbench-epapercard_epapercard | kicad | rs-main | 0.00 | 1 | 0 | 991.1 | +47.8 | | 82.4 | -210.4 | 48 | 2138.9 | win (unrouted) |
| pcbench-esp-leipa_esp-12 | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 59.1 | | 9 | 409.8 | baseline |
| pcbench-esp-leipa_esp-12 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 4.6 | -54.5 | 4 | 443.8 | win (score) |
| pcbench-esp-serial-terminal_esp-com | kicad | java-current | 0.00 | 0 | 18 | 939.0 | | unmeasured | 64.9 | | 14 | 592.6 | baseline |
| pcbench-esp-serial-terminal_esp-com | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +61.0 | | 10.3 | -54.5 | 6 | 616.1 | win (clean_pass_rate) |
| pcbench-esp12-appliance_mod_esp12-appliance-mod | kicad | java-current | 0.00 | 0 | 13 | 960.6 | | unmeasured | 68.6 | | 14 | 758.1 | baseline |
| pcbench-esp12-appliance_mod_esp12-appliance-mod | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +39.4 | | 12.8 | -55.8 | 6 | 795.4 | win (clean_pass_rate) |
| pcbench-esp12-breakout_ESP12Breakout | kicad | java-current | 0.00 | 0 | 3 | 976.9 | | unmeasured | 34.6 | | 0 | 203.8 | baseline |
| pcbench-esp12-breakout_ESP12Breakout | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +23.1 | | 1.4 | -33.2 | 0 | 201.9 | win (clean_pass_rate) |
| pcbench-esp32-ethernet_esp32-ethernet | kicad | java-current | 0.00 | 2 | 1 | 981.3 | | unmeasured | 288.5 | | 57 | 1302.0 | baseline |
| pcbench-esp32-ethernet_esp32-ethernet | kicad | rs-main | 0.00 | 0 | 4 | 993.2 | +11.9 | | 114.2 | -174.2 | 52 | 1558.3 | win (unrouted) |
| pcbench-esp32stack_esp32stack | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 321.6 | | 15 | 1133.6 | baseline |
| pcbench-esp32stack_esp32stack | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 17.5 | -304.1 | 21 | 1118.9 | loss (score) |
| pcbench-esp8266_32x32panel_esp_12_f_595 | kicad | java-current | 0.00 | 1 | 0 | 991.7 | | unmeasured | 254.6 | | 56 | 1656.1 | baseline |
| pcbench-esp8266_32x32panel_esp_12_f_595 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +8.3 | | 105.3 | -149.2 | 41 | 1711.4 | win (clean_pass_rate) |
| pcbench-esp8266_32x32panel_esp_12_f_595_ORDERED | kicad | java-current | 0.00 | 1 | 0 | 991.7 | | unmeasured | 255.7 | | 56 | 1656.1 | baseline |
| pcbench-esp8266_32x32panel_esp_12_f_595_ORDERED | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +8.3 | | 104.3 | -151.3 | 41 | 1711.4 | win (clean_pass_rate) |
| pcbench-esp8266_envmonitor_environment-monitor | kicad | java-current | 0.00 | 2 | 4 | 920.0 | | unmeasured | 89.7 | | 10 | 279.2 | baseline |
| pcbench-esp8266_envmonitor_environment-monitor | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +80.0 | | 2.6 | -87.1 | 2 | 385.9 | win (clean_pass_rate) |
| pcbench-esp8266_envmonitor_environment-monitor-1.2 | kicad | java-current | 0.00 | 0 | 2 | 987.5 | | unmeasured | 54.1 | | 7 | 386.2 | baseline |
| pcbench-esp8266_envmonitor_environment-monitor-1.2 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +12.5 | | 2.0 | -52.1 | 2 | 401.9 | win (clean_pass_rate) |
| pcbench-esp8266_envmonitor_environment-monitor-1.4 | kicad | java-current | 0.00 | 2 | 4 | 920.0 | | unmeasured | 111.0 | | 10 | 279.2 | baseline |
| pcbench-esp8266_envmonitor_environment-monitor-1.4 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +80.0 | | 2.6 | -108.4 | 2 | 385.9 | win (clean_pass_rate) |
| pcbench-esp8266_link_test_esp_micro85-only | kicad | java-current | 0.00 | 5 | 21 | 781.0 | | unmeasured | 124.1 | | 1 | 79.8 | baseline |
| pcbench-esp8266_link_test_esp_micro85-only | kicad | rs-main | 0.00 | 4 | 0 | 904.8 | +123.8 | | 41.3 | -82.8 | 2 | 93.2 | win (unrouted) |
| pcbench-esp8266_network_speaker_esp_network_speaker | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 322.8 | | 29 | 640.5 | baseline |
| pcbench-esp8266_network_speaker_esp_network_speaker | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 40.5 | -282.3 | 22 | 646.8 | win (score) |
| pcbench-esp8266_wi07_3_adapter_esp | kicad | java-current | 0.00 | 0 | 13 | 826.7 | | unmeasured | 25.1 | | 2 | 116.6 | baseline |
| pcbench-esp8266_wi07_3_adapter_esp | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +173.3 | | 0.3 | -24.8 | 2 | 116.5 | win (clean_pass_rate) |
| pcbench-espalarm_alarm | kicad | java-current | 0.00 | 0 | 85 | 831.7 | | unmeasured | 112.8 | | 36 | 1826.1 | baseline |
| pcbench-espalarm_alarm | kicad | rs-main | 0.00 | 0 | 64 | 873.3 | +41.6 | | 39.3 | -73.5 | 18 | 1837.4 | win (violations) |
| pcbench-espeverywhere__autosave-espeverywhere_breakout | kicad | java-current | 0.00 | 0 | 18 | 550.0 | | unmeasured | 22.1 | | 7 | 136.6 | baseline |
| pcbench-espeverywhere__autosave-espeverywhere_breakout | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +450.0 | | 0.4 | -21.6 | 0 | 135.6 | win (clean_pass_rate) |
| pcbench-espionage_esplight | kicad | java-current | 0.00 | 0 | 7 | 961.1 | | unmeasured | 44.2 | | 10 | 403.2 | baseline |
| pcbench-espionage_esplight | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +38.9 | | 5.1 | -39.1 | 6 | 390.3 | win (clean_pass_rate) |
| pcbench-everled_everled | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 34.3 | | 2 | 146.4 | baseline |
| pcbench-everled_everled | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.8 | -33.5 | 2 | 141.1 | win (score) |
| pcbench-ezusb-logicanalyzer_cypress_logic_analyzer | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 44.9 | | 6 | 477.3 | baseline |
| pcbench-ezusb-logicanalyzer_cypress_logic_analyzer | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 6.2 | -38.6 | 3 | 508.5 | win (score) |
| pcbench-f.60_keyboard | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 121.3 | | 0 | 4130.4 | baseline |
| pcbench-f.60_keyboard | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 8.7 | -112.6 | 0 | 4080.4 | win (score) |
| pcbench-fan_controller_fan_controller | kicad | java-current | 0.00 | 0 | 21 | 963.5 | | unmeasured | 79.9 | | 26 | 844.1 | baseline |
| pcbench-fan_controller_fan_controller | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +36.5 | | 31.6 | -48.3 | 17 | 843.1 | win (clean_pass_rate) |
| pcbench-fifogfx_c64cart | kicad | java-current | 0.00 | 3 | 0 | 971.1 | | unmeasured | 167.9 | | 29 | 2734.0 | baseline |
| pcbench-fifogfx_c64cart | kicad | rs-main | 0.00 | 2 | 0 | 980.8 | +9.6 | | 195.3 | +27.4 | 42 | 2851.9 | win (unrouted) |
| pcbench-filament_extruder_sensor | kicad | java-current | 0.00 | 0 | 12 | 885.7 | | unmeasured | 30.1 | | 3 | 367.1 | baseline |
| pcbench-filament_extruder_sensor | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +114.3 | | 2.1 | -27.9 | 1 | 370.2 | win (clean_pass_rate) |
| pcbench-fingerprint-with-esp32_quet van tay | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 36.6 | | 5 | 488.6 | baseline |
| pcbench-fingerprint-with-esp32_quet van tay | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 1.4 | -35.2 | 2 | 486.4 | win (score) |
| pcbench-firefly-jar_solar_lamp | kicad | java-current | 0.00 | 1 | 0 | 950.0 | | unmeasured | 34.4 | | 0 | 268.5 | baseline |
| pcbench-firefly-jar_solar_lamp | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +50.0 | | 0.6 | -33.7 | 0 | 276.1 | win (clean_pass_rate) |
| pcbench-fp2_extension_sample_fp2_usb_breakout | kicad | java-current | 0.00 | 1 | 6 | 633.3 | | unmeasured | 20.4 | | 2 | 40.0 | baseline |
| pcbench-fp2_extension_sample_fp2_usb_breakout | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +366.7 | | 0.8 | -19.6 | 1 | 47.7 | win (clean_pass_rate) |
| pcbench-free-of-charge_BMS | kicad | java-current | 0.00 | 36 | 116 | 790.8 | | unmeasured | 355.4 | | 92 | 1626.7 | baseline |
| pcbench-free-of-charge_BMS | kicad | rs-main | 0.00 | 19 | 0 | 932.9 | +142.0 | | 139.1 | -216.4 | 74 | 1941.5 | win (unrouted) |
| pcbench-freeDSP-CLASSIC-SMD-BALANCED_FreeDSP_BAL | kicad | java-current | 0.00 | 152 | 125 | 625.0 | | unmeasured | 339.8 | | 91 | 1724.7 | baseline |
| pcbench-freeDSP-CLASSIC-SMD-BALANCED_FreeDSP_BAL | kicad | rs-main | 0.00 | 7 | 0 | 985.2 | +360.2 | | 177.3 | -162.5 | 58 | 4039.6 | win (unrouted) |
| pcbench-freeUSBi_USBi_Programmer | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 51.1 | | 3 | 543.0 | baseline |
| pcbench-freeUSBi_USBi_Programmer | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 3.9 | -47.3 | 1 | 538.6 | win (score) |
| pcbench-gdrom_adapter_board_adapter | kicad | java-current | 0.00 | 2 | 0 | 981.6 | | unmeasured | 270.4 | | 56 | 3172.4 | baseline |
| pcbench-gdrom_adapter_board_adapter | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +18.3 | | 132.3 | -138.2 | 39 | 3423.3 | win (clean_pass_rate) |
| pcbench-gepetto_circuito | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 48.5 | | 2 | 974.1 | baseline |
| pcbench-gepetto_circuito | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 3.3 | -45.1 | 0 | 995.1 | win (score) |
| pcbench-guitar_fret | kicad | java-current | 0.00 | 2 | 32 | 916.0 | | unmeasured | 236.7 | | 54 | 1001.5 | baseline |
| pcbench-guitar_fret | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +84.0 | | 90.8 | -146.0 | 29 | 1021.7 | win (clean_pass_rate) |
| pcbench-gwurrbus_pwm | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 39.8 | | 0 | 1063.1 | baseline |
| pcbench-gwurrbus_pwm | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 1.3 | -38.4 | 0 | 1054.1 | win (score) |
| pcbench-hackaday_esp-14_power_meter__autosave-esp-14 | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 17.7 | | 2 | 161.7 | baseline |
| pcbench-hackaday_esp-14_power_meter__autosave-esp-14 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.2 | -17.5 | 0 | 161.5 | win (score) |
| pcbench-hackyflasher_Flasher | kicad | java-current | 0.00 | 1 | 0 | 937.5 | | unmeasured | 43.2 | | 0 | 601.8 | baseline |
| pcbench-hackyflasher_Flasher | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +62.5 | | 0.1 | -43.1 | 0 | 664.0 | win (clean_pass_rate) |
| pcbench-hardware-designs_c-trigger | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 12.8 | | 0 | 86.3 | baseline |
| pcbench-hardware-designs_c-trigger | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.1 | -12.7 | 0 | 84.5 | win (score) |
| pcbench-hardware-designs_m-trigger | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 14.0 | | 0 | 106.7 | baseline |
| pcbench-hardware-designs_m-trigger | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.1 | -14.0 | 0 | 105.3 | win (score) |
| pcbench-hardware-designs_nixie-combo | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 99.3 | | 14 | 2862.9 | baseline |
| pcbench-hardware-designs_nixie-combo | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 27.2 | -72.2 | 3 | 2848.5 | win (score) |
| pcbench-hardware-designs_nixie-power | kicad | java-current | 0.00 | 0 | 5 | 974.4 | | unmeasured | 29.7 | | 10 | 471.9 | baseline |
| pcbench-hardware-designs_nixie-power | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +25.6 | | 3.3 | -26.4 | 2 | 470.7 | win (clean_pass_rate) |
| pcbench-hardware-designs_soil-moisture-sensor | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 44.9 | | 10 | 430.7 | baseline |
| pcbench-hardware-designs_soil-moisture-sensor | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 3.6 | -41.2 | 5 | 487.4 | win (score) |
| pcbench-hardware-designs_solar-harvester | kicad | java-current | 0.00 | 0 | 1 | 992.6 | | unmeasured | 22.9 | | 5 | 235.7 | baseline |
| pcbench-hardware-designs_solar-harvester | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +7.4 | | 2.0 | -20.9 | 2 | 225.2 | win (clean_pass_rate) |
| pcbench-hardware-designs_spsgrf-board | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 25.5 | | 5 | 167.4 | baseline |
| pcbench-hardware-designs_spsgrf-board | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 1.1 | -24.4 | 5 | 170.0 | loss (score) |
| pcbench-headstage-adapter_headstage adapter | kicad | java-current | 0.00 | 13 | 74 | 747.3 | | unmeasured | 361.5 | | 124 | 1592.0 | baseline |
| pcbench-headstage-adapter_headstage adapter | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +252.7 | | 189.2 | -172.3 | 60 | 1873.4 | win (clean_pass_rate) |
| pcbench-helmholtz-servo_CurrentServo | kicad | java-current | 0.00 | 1 | 22 | 975.3 | | unmeasured | 211.6 | | 48 | 2539.7 | baseline |
| pcbench-helmholtz-servo_CurrentServo | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +24.7 | | 104.9 | -106.7 | 13 | 2740.4 | win (clean_pass_rate) |
| pcbench-hm-mod-rpi-rtc_hm-mod-rpi-rtc | kicad | java-current | 0.00 | 0 | 3 | 987.2 | | unmeasured | 55.1 | | 6 | 348.3 | baseline |
| pcbench-hm-mod-rpi-rtc_hm-mod-rpi-rtc | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +12.8 | | 7.8 | -47.2 | 5 | 366.4 | win (clean_pass_rate) |
| pcbench-hw_trials_demo | kicad | java-current | 0.00 | 1 | 6 | 975.5 | | unmeasured | 126.4 | | 30 | 1025.7 | baseline |
| pcbench-hw_trials_demo | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +24.4 | | 17.9 | -108.5 | 11 | 1165.5 | win (clean_pass_rate) |
| pcbench-hwstar_ac-power-monitor | kicad | java-current | 0.00 | 1 | 13 | 966.3 | | unmeasured | 148.4 | | 37 | 1110.7 | baseline |
| pcbench-hwstar_ac-power-monitor | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +33.6 | | 79.6 | -68.9 | 12 | 1296.4 | win (clean_pass_rate) |
| pcbench-icehat_icehat | kicad | java-current | 0.00 | 16 | 2 | 877.6 | | unmeasured | 370.9 | | 37 | 1132.8 | baseline |
| pcbench-icehat_icehat | kicad | rs-main | 0.00 | 9 | 0 | 932.8 | +55.2 | | 132.7 | -238.2 | 28 | 1256.2 | win (unrouted) |
| pcbench-imfr-schematics_Telescopio | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 67.4 | | 0 | 1344.7 | baseline |
| pcbench-imfr-schematics_Telescopio | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 1.9 | -65.5 | 0 | 1351.9 | loss (score) |
| pcbench-induction-hob_temperature-sender | kicad | java-current | 0.00 | 3 | 12 | 937.2 | | unmeasured | 163.6 | | 24 | 613.6 | baseline |
| pcbench-induction-hob_temperature-sender | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +62.8 | | 32.5 | -131.2 | 21 | 692.7 | win (clean_pass_rate) |
| pcbench-jadonk_PocketBone | kicad | java-current | 0.00 | 45 | 8 | 769.3 | | unmeasured | 373.4 | | 7 | 863.5 | baseline |
| pcbench-jadonk_PocketBone | kicad | rs-main | 0.00 | 45 | 0 | 777.2 | +7.9 | | 185.4 | -188.0 | 8 | 967.4 | win (violations) |
| pcbench-jdy-08-board_jdy-08 | kicad | java-current | 0.00 | 1 | 35 | 724.1 | | unmeasured | 76.0 | | 4 | 411.2 | baseline |
| pcbench-jdy-08-board_jdy-08 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +275.9 | | 2.9 | -73.1 | 1 | 417.3 | win (clean_pass_rate) |
| pcbench-juno-chorus-clone_juno-chorus-clone | kicad | java-current | 0.00 | 1 | 3 | 994.7 | | unmeasured | 254.7 | | 29 | 4419.2 | baseline |
| pcbench-juno-chorus-clone_juno-chorus-clone | kicad | rs-main | 0.00 | 5 | 4 | 980.7 | -14.0 | | 85.2 | -169.6 | 35 | 4243.7 | loss (unrouted) |
| pcbench-karabas-nano_karabas-nano-revA | kicad | java-current | 0.00 | 272 | 159 | 497.8 | | unmeasured | 400.8 | | 267 | 2729.6 | baseline |
| pcbench-karabas-nano_karabas-nano-revA | kicad | rs-main | 0.00 | 108 | 0 | 821.5 | +323.6 | | 299.8 | -101.0 | 309 | 5985.5 | win (unrouted) |
| pcbench-karabas-nano_karabas-nano-revB | kicad | java-current | 0.00 | 275 | 199 | 486.5 | | unmeasured | 383.3 | | 276 | 2692.2 | baseline |
| pcbench-karabas-nano_karabas-nano-revB | kicad | rs-main | 0.00 | 91 | 0 | 851.5 | +365.1 | | 299.9 | -83.5 | 307 | 6367.0 | win (unrouted) |
| pcbench-karabas-nano_karabas-nano-revC | kicad | java-current | 0.00 | 334 | 1 | 499.7 | | unmeasured | 378.4 | | 345 | 2283.6 | baseline |
| pcbench-karabas-nano_karabas-nano-revC | kicad | rs-main | 0.00 | 96 | 1 | 856.0 | +356.3 | | 300.5 | -77.9 | 308 | 6647.9 | win (unrouted) |
| pcbench-karabas-nano_karabas-nano-revG | kicad | java-current | 0.00 | 323 | 0 | 491.3 | | unmeasured | 367.4 | | 348 | 2284.7 | baseline |
| pcbench-karabas-nano_karabas-nano-revG | kicad | rs-main | 0.00 | 99 | 0 | 844.1 | +352.8 | | 300.2 | -67.2 | 313 | 6857.0 | win (unrouted) |
| pcbench-karabas-nano_wifi_revA | kicad | java-current | 0.00 | 0 | 9 | 914.3 | | unmeasured | 42.0 | | 3 | 292.6 | baseline |
| pcbench-karabas-nano_wifi_revA | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +85.7 | | 1.7 | -40.3 | 4 | 278.6 | win (clean_pass_rate) |
| pcbench-kassenautomat.mdb-interface_mdb-interface | kicad | java-current | 0.00 | 0 | 68 | 917.1 | | unmeasured | 169.6 | | 36 | 2139.9 | baseline |
| pcbench-kassenautomat.mdb-interface_mdb-interface | kicad | rs-main | 0.00 | 1 | 0 | 993.9 | +76.8 | | 51.5 | -118.1 | 26 | 2155.3 | loss (unrouted) |
| pcbench-kicad-guitar-preamp_Preamp-Instructables | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 17.5 | | 0 | 147.3 | baseline |
| pcbench-kicad-guitar-preamp_Preamp-Instructables | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.3 | -17.2 | 0 | 146.2 | win (score) |
| pcbench-kicad-projects_BatCharge | kicad | java-current | 0.00 | 0 | 4 | 963.6 | | unmeasured | 35.5 | | 6 | 125.2 | baseline |
| pcbench-kicad-projects_BatCharge | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +36.4 | | 3.9 | -31.6 | 2 | 136.1 | win (clean_pass_rate) |
| pcbench-kicad-projects_ili9341-breakout | kicad | java-current | 0.00 | 3 | 0 | 833.3 | | unmeasured | 66.1 | | 10 | 233.3 | baseline |
| pcbench-kicad-projects_ili9341-breakout | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +166.7 | | 1.8 | -64.3 | 0 | 262.0 | win (clean_pass_rate) |
| pcbench-kicad_bbb-melzi | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 44.1 | | 2 | 421.8 | baseline |
| pcbench-kicad_bbb-melzi | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 1.8 | -42.3 | 0 | 400.6 | win (score) |
| pcbench-kika-in-space_DS8500 | kicad | java-current | 0.00 | 0 | 10 | 954.5 | | unmeasured | 38.2 | | 4 | 229.0 | baseline |
| pcbench-kika-in-space_DS8500 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +45.5 | | 5.4 | -32.8 | 1 | 271.4 | win (clean_pass_rate) |
| pcbench-kika-in-space_analog-test-board | kicad | java-current | 0.00 | 0 | 19 | 873.3 | | unmeasured | 33.2 | | 6 | 159.0 | baseline |
| pcbench-kika-in-space_analog-test-board | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +126.7 | | 1.8 | -31.4 | 0 | 173.6 | win (clean_pass_rate) |
| pcbench-kit2-led-cube_led_cube | kicad | java-current | 0.00 | 0 | 7 | 986.5 | | unmeasured | 351.6 | | 2 | 989.0 | baseline |
| pcbench-kit2-led-cube_led_cube | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +13.5 | | 66.1 | -285.4 | 6 | 939.1 | win (clean_pass_rate) |
| pcbench-kitspace_12V5A_breakout | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 20.8 | | 0 | 222.6 | baseline |
| pcbench-kitspace_12V5A_breakout | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.3 | -20.5 | 0 | 222.6 | win (score) |
| pcbench-kitspace_12_24_boost_converter | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 22.1 | | 0 | 310.6 | baseline |
| pcbench-kitspace_12_24_boost_converter | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 1.3 | -20.8 | 0 | 308.0 | win (score) |
| pcbench-kitspace_40-channel-hv-switching-board | kicad | java-current | 0.00 | 172 | 22 | 707.9 | | unmeasured | 369.7 | | 204 | 3027.9 | baseline |
| pcbench-kitspace_40-channel-hv-switching-board | kicad | rs-main | 0.00 | 1 | 0 | 998.3 | +290.4 | | 301.0 | -68.6 | 272 | 7704.8 | win (unrouted) |
| pcbench-kitspace_4_switch_array | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 16.8 | | 0 | 303.0 | baseline |
| pcbench-kitspace_4_switch_array | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.2 | -16.6 | 0 | 302.9 | win (score) |
| pcbench-kitspace_8_switch_array | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 33.1 | | 0 | 578.9 | baseline |
| pcbench-kitspace_8_switch_array | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 3.8 | -29.3 | 0 | 566.4 | win (score) |
| pcbench-kitspace_BQ25570_Harvester | kicad | java-current | 0.00 | 6 | 2 | 881.5 | | unmeasured | 130.1 | | 5 | 138.1 | baseline |
| pcbench-kitspace_BQ25570_Harvester | kicad | rs-main | 0.00 | 7 | 0 | 870.4 | -11.1 | | 44.7 | -85.5 | 0 | 153.6 | loss (unrouted) |
| pcbench-kitspace_CH330 | kicad | java-current | 0.00 | 2 | 3 | 891.7 | | unmeasured | 78.2 | | 3 | 68.8 | baseline |
| pcbench-kitspace_CH330 | kicad | rs-main | 0.00 | 1 | 0 | 958.3 | +66.7 | | 27.5 | -50.7 | 4 | 74.4 | win (unrouted) |
| pcbench-kitspace_CO2 | kicad | java-current | 0.00 | 0 | 18 | 960.0 | | unmeasured | 131.8 | | 15 | 745.5 | baseline |
| pcbench-kitspace_CO2 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +40.0 | | 23.9 | -107.9 | 15 | 726.1 | win (clean_pass_rate) |
| pcbench-kitspace_DIY_detector | kicad | java-current | 0.00 | 0 | 4 | 983.7 | | unmeasured | 80.7 | | 0 | 407.1 | baseline |
| pcbench-kitspace_DIY_detector | kicad | rs-main | 0.00 | 0 | 1 | 995.9 | +12.2 | | 4.4 | -76.3 | 0 | 387.6 | win (violations) |
| pcbench-kitspace_Lcr_addon | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 50.4 | | 2 | 803.9 | baseline |
| pcbench-kitspace_Lcr_addon | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 4.0 | -46.4 | 0 | 803.8 | win (score) |
| pcbench-kitspace_Minisumo_V2.1 | kicad | java-current | 0.00 | 1 | 0 | 979.2 | | unmeasured | 162.4 | | 21 | 1036.2 | baseline |
| pcbench-kitspace_Minisumo_V2.1 | kicad | rs-main | 0.00 | 1 | 0 | 979.2 | -0.0 | | 70.0 | -92.4 | 22 | 1023.0 | loss (score) |
| pcbench-kitspace_OSO-BOOK-C1 | kicad | java-current | 0.00 | 5 | 34 | 917.5 | | unmeasured | 331.0 | | 69 | 2245.2 | baseline |
| pcbench-kitspace_OSO-BOOK-C1 | kicad | rs-main | 0.00 | 7 | 31 | 907.7 | -9.8 | | 208.8 | -122.3 | 51 | 2057.8 | loss (unrouted) |
| pcbench-kitspace_OtterScreen | kicad | java-current | 0.00 | 0 | 17 | 971.9 | | unmeasured | 149.2 | | 31 | 735.5 | baseline |
| pcbench-kitspace_OtterScreen | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +28.1 | | 62.1 | -87.2 | 25 | 705.5 | win (clean_pass_rate) |
| pcbench-kitspace_PSLab | kicad | java-current | 0.00 | 13 | 86 | 910.9 | | unmeasured | 381.9 | | 167 | 2848.8 | baseline |
| pcbench-kitspace_PSLab | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +89.1 | | 121.1 | -260.8 | 149 | 3329.9 | win (clean_pass_rate) |
| pcbench-kitspace_Potentiometer_mount_4LED | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 16.1 | | 0 | 204.9 | baseline |
| pcbench-kitspace_Potentiometer_mount_4LED | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.1 | -16.1 | 0 | 204.6 | win (score) |
| pcbench-kitspace_Potentiometer_mount_8LED | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 29.8 | | 0 | 330.6 | baseline |
| pcbench-kitspace_Potentiometer_mount_8LED | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.3 | -29.5 | 0 | 313.2 | win (score) |
| pcbench-kitspace_RPi_shield | kicad | java-current | 0.00 | 1 | 0 | 954.5 | | unmeasured | 42.1 | | 0 | 325.8 | baseline |
| pcbench-kitspace_RPi_shield | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +45.5 | | 1.1 | -40.9 | 0 | 331.9 | win (clean_pass_rate) |
| pcbench-kitspace_T32_ref | kicad | java-current | 0.00 | 0 | 15 | 978.1 | | unmeasured | 370.1 | | 84 | 2063.2 | baseline |
| pcbench-kitspace_T32_ref | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +21.9 | | 298.9 | -71.2 | 66 | 2101.3 | win (clean_pass_rate) |
| pcbench-kitspace_USB-C-Screen-Adapter | kicad | java-current | 0.00 | 12 | 10 | 929.6 | | unmeasured | 385.3 | | 83 | 1048.6 | baseline |
| pcbench-kitspace_USB-C-Screen-Adapter | kicad | rs-main | 0.00 | 4 | 3 | 976.9 | +47.2 | | 139.4 | -245.8 | 86 | 1076.6 | win (unrouted) |
| pcbench-kitspace_USB-C-Screen-Adapter-LDR6023SS | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 184.9 | | 46 | 894.1 | baseline |
| pcbench-kitspace_USB-C-Screen-Adapter-LDR6023SS | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 136.4 | -48.5 | 41 | 903.8 | win (score) |
| pcbench-kitspace__autosave-nunchuk_breakout | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 35.0 | | 0 | 150.9 | baseline |
| pcbench-kitspace__autosave-nunchuk_breakout | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 0.6 | -34.4 | 0 | 153.5 | loss (score) |
| pcbench-kitspace_aquarius | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 264.9 | | 61 | 2430.3 | baseline |
| pcbench-kitspace_aquarius | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 106.2 | -158.7 | 58 | 2431.3 | win (score) |
| pcbench-kitspace_ardfpga | kicad | java-current | 0.00 | 25 | 54 | 828.7 | | unmeasured | 393.4 | | 114 | 2078.9 | baseline |
| pcbench-kitspace_ardfpga | kicad | rs-main | 0.00 | 10 | 0 | 952.1 | +123.4 | | 155.2 | -238.3 | 109 | 2379.0 | win (unrouted) |
| pcbench-kitspace_beehive | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 55.3 | | 0 | 1645.3 | baseline |
| pcbench-kitspace_beehive | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 6.7 | -48.6 | 0 | 1633.1 | win (score) |
| pcbench-kitspace_dropbot-front-panel | kicad | java-current | 0.00 | 67 | 6 | 667.3 | | unmeasured | 411.4 | | 123 | 4682.1 | baseline |
| pcbench-kitspace_dropbot-front-panel | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +332.7 | | 299.0 | -112.5 | 172 | 8616.3 | win (clean_pass_rate) |
| pcbench-kitspace_dropbot_control_board | kicad | java-current | 0.00 | 6 | 59 | 941.6 | | unmeasured | 403.3 | | 60 | 2892.0 | baseline |
| pcbench-kitspace_dropbot_control_board | kicad | rs-main | 0.00 | 2 | 5 | 990.2 | +48.5 | | 229.1 | -174.2 | 45 | 3049.7 | win (unrouted) |
| pcbench-kitspace_dynamixel_shield | kicad | java-current | 0.00 | 5 | 8 | 905.7 | | unmeasured | 143.4 | | 11 | 807.7 | baseline |
| pcbench-kitspace_dynamixel_shield | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +94.3 | | 17.0 | -126.4 | 4 | 886.5 | win (clean_pass_rate) |
| pcbench-kitspace_esp8266 | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 54.3 | | 0 | 613.6 | baseline |
| pcbench-kitspace_esp8266 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 3.3 | -51.0 | 0 | 596.3 | win (score) |
| pcbench-kitspace_flypi | kicad | java-current | 0.00 | 0 | 20 | 970.1 | | unmeasured | 353.5 | | 54 | 2481.6 | baseline |
| pcbench-kitspace_flypi | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +29.9 | | 75.5 | -277.9 | 22 | 2590.4 | win (clean_pass_rate) |
| pcbench-kitspace_flypi_v2 | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 81.0 | | 2 | 1872.3 | baseline |
| pcbench-kitspace_flypi_v2 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 29.8 | -51.2 | 0 | 1836.5 | win (score) |
| pcbench-kitspace_gas_sensor | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 24.7 | | 0 | 238.7 | baseline |
| pcbench-kitspace_gas_sensor | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.6 | -24.1 | 0 | 238.4 | win (score) |
| pcbench-kitspace_grove_adaptor | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 13.0 | | 0 | 17.7 | baseline |
| pcbench-kitspace_grove_adaptor | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.0 | -13.0 | 0 | 17.7 | win (cpu_s) |
| pcbench-kitspace_hbridge_driver | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 46.2 | | 0 | 587.6 | baseline |
| pcbench-kitspace_hbridge_driver | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 3.4 | -42.8 | 0 | 570.5 | win (score) |
| pcbench-kitspace_hp_led_switch | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 39.1 | | 0 | 562.2 | baseline |
| pcbench-kitspace_hp_led_switch | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 3.3 | -35.7 | 0 | 545.9 | win (score) |
| pcbench-kitspace_hum_temp_sensor | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 14.6 | | 0 | 91.2 | baseline |
| pcbench-kitspace_hum_temp_sensor | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 0.1 | -14.5 | 0 | 94.8 | loss (score) |
| pcbench-kitspace_ideal_diode | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 122.6 | | 2 | 212.3 | baseline |
| pcbench-kitspace_ideal_diode | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 4.5 | -118.1 | 4 | 190.4 | loss (score) |
| pcbench-kitspace_ir_sensor | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 14.9 | | 0 | 228.3 | baseline |
| pcbench-kitspace_ir_sensor | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.1 | -14.8 | 0 | 228.1 | win (score) |
| pcbench-kitspace_led_driver | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 120.7 | | 2 | 627.5 | baseline |
| pcbench-kitspace_led_driver | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 4.7 | -116.0 | 1 | 596.8 | win (score) |
| pcbench-kitspace_level_shifter | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 66.2 | | 0 | 474.0 | baseline |
| pcbench-kitspace_level_shifter | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 1.5 | -64.8 | 0 | 453.7 | win (score) |
| pcbench-kitspace_minisumo_v3 | kicad | java-current | 0.00 | 2 | 0 | 986.4 | | unmeasured | 352.0 | | 73 | 2587.1 | baseline |
| pcbench-kitspace_minisumo_v3 | kicad | rs-main | 0.00 | 2 | 3 | 982.3 | -4.1 | | 128.5 | -223.5 | 64 | 2616.5 | loss (violations) |
| pcbench-kitspace_nunchuk_breakout | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 57.6 | | 0 | 167.9 | baseline |
| pcbench-kitspace_nunchuk_breakout | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.7 | -56.9 | 0 | 155.2 | win (score) |
| pcbench-kitspace_peltier | kicad | java-current | 0.00 | 1 | 0 | 985.1 | | unmeasured | 67.3 | | 0 | 635.8 | baseline |
| pcbench-kitspace_peltier | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +14.9 | | 9.7 | -57.6 | 0 | 656.1 | win (clean_pass_rate) |
| pcbench-kitspace_piezo_amplifier | kicad | java-current | 0.00 | 2 | 0 | 973.0 | | unmeasured | 144.8 | | 17 | 3545.7 | baseline |
| pcbench-kitspace_piezo_amplifier | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +27.0 | | 51.9 | -93.0 | 11 | 3799.1 | win (clean_pass_rate) |
| pcbench-kitspace_pmt_combiner | kicad | java-current | 0.00 | 0 | 13 | 925.7 | | unmeasured | 31.2 | | 0 | 581.3 | baseline |
| pcbench-kitspace_pmt_combiner | kicad | rs-main | 0.00 | 0 | 1 | 994.3 | +68.6 | | 0.6 | -30.6 | 0 | 578.5 | win (violations) |
| pcbench-kitspace_power_supply | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 25.1 | | 0 | 312.6 | baseline |
| pcbench-kitspace_power_supply | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 2.0 | -23.1 | 0 | 296.6 | win (score) |
| pcbench-kitspace_sensor | kicad | java-current | 0.00 | 2 | 128 | 854.7 | | unmeasured | 314.9 | | 73 | 2082.0 | baseline |
| pcbench-kitspace_sensor | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +145.3 | | 166.7 | -148.2 | 39 | 2080.6 | win (clean_pass_rate) |
| pcbench-kitspace_solenoid_driver | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 23.4 | | 0 | 368.1 | baseline |
| pcbench-kitspace_solenoid_driver | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 1.3 | -22.1 | 0 | 347.4 | win (score) |
| pcbench-kitspace_spike_n_hold | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 53.7 | | 1 | 1099.1 | baseline |
| pcbench-kitspace_spike_n_hold | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 2.3 | -51.4 | 0 | 1113.4 | win (score) |
| pcbench-kitspace_sympetrum-v2%20NFF1.1 | kicad | java-current | 0.00 | 14 | 55 | 782.6 | | unmeasured | 325.9 | | 42 | 835.0 | baseline |
| pcbench-kitspace_sympetrum-v2%20NFF1.1 | kicad | rs-main | 0.00 | 14 | 0 | 878.3 | +95.7 | | 87.5 | -238.4 | 28 | 821.6 | win (violations) |
| pcbench-kitspace_teensy-fx | kicad | java-current | 0.00 | 29 | 54 | 796.9 | | unmeasured | 340.3 | | 140 | 2639.1 | baseline |
| pcbench-kitspace_teensy-fx | kicad | rs-main | 0.00 | 5 | 0 | 974.5 | +177.5 | | 281.6 | -58.8 | 143 | 3587.5 | win (unrouted) |
| pcbench-kitspace_temp_breakout | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 22.4 | | 0 | 255.0 | baseline |
| pcbench-kitspace_temp_breakout | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.2 | -22.2 | 0 | 251.7 | win (score) |
| pcbench-kitspace_threeboard | kicad | java-current | 0.00 | 0 | 23 | 961.3 | | unmeasured | 109.1 | | 31 | 1295.2 | baseline |
| pcbench-kitspace_threeboard | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +38.7 | | 40.6 | -68.4 | 28 | 1228.1 | win (clean_pass_rate) |
| pcbench-kitspace_training_board_v02 | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 66.7 | | 0 | 1417.4 | baseline |
| pcbench-kitspace_training_board_v02 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 17.3 | -49.4 | 0 | 1396.2 | win (score) |
| pcbench-kitspace_trans_switch_volt_amp | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 35.1 | | 0 | 804.6 | baseline |
| pcbench-kitspace_trans_switch_volt_amp | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 1.8 | -33.3 | 0 | 809.1 | loss (score) |
| pcbench-kitspace_tt_nano_HAT_b1 | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 32.6 | | 0 | 469.2 | baseline |
| pcbench-kitspace_tt_nano_HAT_b1 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 1.9 | -30.8 | 0 | 463.9 | win (score) |
| pcbench-kitspace_tt_nano_HAT_b2 | kicad | java-current | 0.00 | 1 | 0 | 982.1 | | unmeasured | 73.3 | | 0 | 616.1 | baseline |
| pcbench-kitspace_tt_nano_HAT_b2 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +17.9 | | 3.4 | -69.9 | 0 | 654.7 | win (clean_pass_rate) |
| pcbench-kitspace_tt_opt101_module_b1 | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 14.8 | | 0 | 56.6 | baseline |
| pcbench-kitspace_tt_opt101_module_b1 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.1 | -14.7 | 0 | 56.6 | win (cpu_s) |
| pcbench-klangorium_logic_noise_playground | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 226.8 | | 16 | 4173.9 | baseline |
| pcbench-klangorium_logic_noise_playground | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 31.0 | -195.8 | 4 | 4282.9 | win (score) |
| pcbench-komputer-klavier_KomputerKlavier | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 59.5 | | 1 | 759.3 | baseline |
| pcbench-komputer-klavier_KomputerKlavier | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 17.5 | -42.0 | 0 | 739.8 | win (score) |
| pcbench-led-wordclock_wordclock | kicad | java-current | 0.00 | 7 | 63 | 879.7 | | unmeasured | 333.7 | | 49 | 1547.4 | baseline |
| pcbench-led-wordclock_wordclock | kicad | rs-main | 0.00 | 5 | 4 | 964.4 | +84.7 | | 67.0 | -266.6 | 52 | 1608.6 | win (unrouted) |
| pcbench-led_array_atmega8_led_array | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 76.8 | | 0 | 1533.1 | baseline |
| pcbench-led_array_atmega8_led_array | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 10.4 | -66.4 | 0 | 1481.8 | win (score) |
| pcbench-light-painting-wand_light-wand | kicad | java-current | 0.00 | 0 | 42 | 883.3 | | unmeasured | 120.9 | | 24 | 523.2 | baseline |
| pcbench-light-painting-wand_light-wand | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +116.7 | | 23.3 | -97.7 | 16 | 579.7 | win (clean_pass_rate) |
| pcbench-linklayer_contact | kicad | java-current | 0.00 | 1 | 32 | 913.9 | | unmeasured | 136.0 | | 22 | 709.7 | baseline |
| pcbench-linklayer_contact | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +86.0 | | 17.7 | -118.3 | 8 | 766.0 | win (clean_pass_rate) |
| pcbench-low-power-counter_lpcounter | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 147.3 | | 0 | 636.3 | baseline |
| pcbench-low-power-counter_lpcounter | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 11.3 | -136.0 | 1 | 617.6 | loss (score) |
| pcbench-m2-electronics_m2fc | kicad | java-current | 0.00 | 131 | 201 | 764.5 | | unmeasured | 373.8 | | 13 | 1783.4 | baseline |
| pcbench-m2-electronics_m2fc | kicad | rs-main | 0.00 | 34 | 4 | 952.1 | +187.6 | | 301.5 | -72.3 | 102 | 3166.0 | win (unrouted) |
| pcbench-m2-electronics_m2pogo | kicad | java-current | 0.00 | 0 | 4 | 800.0 | | unmeasured | 14.9 | | 0 | 52.4 | baseline |
| pcbench-m2-electronics_m2pogo | kicad | rs-main | 0.00 | 0 | 4 | 800.0 | 0.0 | | 0.0 | -14.9 | 0 | 52.4 | win (cpu_s) |
| pcbench-m2-electronics_m2r | kicad | java-current | 0.00 | 1 | 40 | 957.5 | | unmeasured | 355.9 | | 57 | 1255.6 | baseline |
| pcbench-m2-electronics_m2r | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +42.5 | | 170.3 | -185.6 | 24 | 1362.8 | win (clean_pass_rate) |
| pcbench-m2-electronics_m2rl | kicad | java-current | 0.00 | 0 | 2 | 992.9 | | unmeasured | 53.1 | | 7 | 312.8 | baseline |
| pcbench-m2-electronics_m2rl | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +7.1 | | 5.8 | -47.3 | 5 | 325.3 | win (clean_pass_rate) |
| pcbench-mac-pro-conversion_front-panel-power-adapter | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 35.8 | | 0 | 195.4 | baseline |
| pcbench-mac-pro-conversion_front-panel-power-adapter | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 0.2 | -35.6 | 1 | 204.6 | loss (score) |
| pcbench-magic-table_etch-a-sketch_cyclone | kicad | java-current | 0.00 | 1 | 0 | 981.5 | | unmeasured | 62.1 | | 1 | 785.9 | baseline |
| pcbench-magic-table_etch-a-sketch_cyclone | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +18.5 | | 3.0 | -59.1 | 0 | 806.4 | win (clean_pass_rate) |
| pcbench-makerspace-emonth_resistor_board | kicad | java-current | 0.00 | 1 | 0 | 975.0 | | unmeasured | 102.5 | | 0 | 836.9 | baseline |
| pcbench-makerspace-emonth_resistor_board | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +25.0 | | 3.2 | -99.3 | 0 | 923.9 | win (clean_pass_rate) |
| pcbench-marlin-neopixel-bridge_ATtiny85_Marneo | kicad | java-current | 0.00 | 3 | 0 | 812.5 | | unmeasured | 32.4 | | 0 | 98.1 | baseline |
| pcbench-marlin-neopixel-bridge_ATtiny85_Marneo | kicad | rs-main | 0.00 | 3 | 0 | 812.5 | +0.0 | | 1.5 | -30.9 | 0 | 91.1 | win (score) |
| pcbench-mavbridge_mavbridge | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 114.2 | | 27 | 525.1 | baseline |
| pcbench-mavbridge_mavbridge | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 28.6 | -85.6 | 19 | 532.8 | win (score) |
| pcbench-maytal_Maytal | kicad | java-current | 0.00 | 1 | 0 | 950.0 | | unmeasured | 51.1 | | 2 | 485.3 | baseline |
| pcbench-maytal_Maytal | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +50.0 | | 1.2 | -49.8 | 0 | 516.5 | win (clean_pass_rate) |
| pcbench-mdbwerk_mdbwerk | kicad | java-current | 0.00 | 5 | 6 | 927.9 | | unmeasured | 191.1 | | 23 | 543.6 | baseline |
| pcbench-mdbwerk_mdbwerk | kicad | rs-main | 0.00 | 3 | 2 | 960.5 | +32.6 | | 46.2 | -144.9 | 25 | 488.7 | win (unrouted) |
| pcbench-mearm-base-pcb_ServoPCB | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 44.7 | | 3 | 408.3 | baseline |
| pcbench-mearm-base-pcb_ServoPCB | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 5.6 | -39.1 | 1 | 409.6 | win (score) |
| pcbench-mechkeys_lfk78-jtag | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 19.5 | | 0 | 118.6 | baseline |
| pcbench-mechkeys_lfk78-jtag | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 0.1 | -19.4 | 0 | 121.8 | loss (score) |
| pcbench-medusa_medusa_rs422_rx | kicad | java-current | 0.00 | 0 | 35 | 927.8 | | unmeasured | 82.8 | | 21 | 650.1 | baseline |
| pcbench-medusa_medusa_rs422_rx | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +72.2 | | 17.4 | -65.4 | 3 | 678.1 | win (clean_pass_rate) |
| pcbench-memory-display_memory-display | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 34.3 | | 1 | 511.0 | baseline |
| pcbench-memory-display_memory-display | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 1.6 | -32.7 | 1 | 511.6 | loss (score) |
| pcbench-memsarray_mems_array | kicad | java-current | 0.00 | 1 | 0 | 995.9 | | unmeasured | 355.1 | | 73 | 8726.7 | baseline |
| pcbench-memsarray_mems_array | kicad | rs-main | 0.00 | 1 | 0 | 995.9 | -0.0 | | 216.8 | -138.3 | 79 | 8916.8 | loss (score) |
| pcbench-microphone_preamp | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 41.3 | | 10 | 339.5 | baseline |
| pcbench-microphone_preamp | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 10.3 | -31.0 | 4 | 338.3 | win (score) |
| pcbench-mightyduino_mightyduino | kicad | java-current | 0.00 | 6 | 0 | 925.0 | | unmeasured | 269.3 | | 38 | 936.6 | baseline |
| pcbench-mightyduino_mightyduino | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +75.0 | | 93.5 | -175.8 | 41 | 1052.6 | win (clean_pass_rate) |
| pcbench-mini_ice40_mini_ice40 | kicad | java-current | 0.00 | 0 | 80 | 875.0 | | unmeasured | 174.0 | | 51 | 955.2 | baseline |
| pcbench-mini_ice40_mini_ice40 | kicad | rs-main | 0.00 | 0 | 15 | 976.6 | +101.6 | | 153.7 | -20.3 | 17 | 974.7 | win (violations) |
| pcbench-miniboard-opamp_miniboard-opamp | kicad | java-current | 0.00 | 0 | 2 | 990.5 | | unmeasured | 132.7 | | 13 | 310.6 | baseline |
| pcbench-miniboard-opamp_miniboard-opamp | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +9.5 | | 8.1 | -124.6 | 8 | 278.4 | win (clean_pass_rate) |
| pcbench-miniboard-stm32f0_miniboard-stm32f0 | kicad | java-current | 0.00 | 0 | 9 | 971.9 | | unmeasured | 249.2 | | 25 | 562.6 | baseline |
| pcbench-miniboard-stm32f0_miniboard-stm32f0 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +28.1 | | 25.0 | -224.2 | 20 | 528.5 | win (clean_pass_rate) |
| pcbench-mobile-sensor-pcb_mobile-sensor-pcb | kicad | java-current | 0.00 | 2 | 6 | 985.6 | | unmeasured | 375.2 | | 93 | 3356.3 | baseline |
| pcbench-mobile-sensor-pcb_mobile-sensor-pcb | kicad | rs-main | 0.00 | 1 | 4 | 991.9 | +6.3 | | 158.5 | -216.7 | 61 | 3620.7 | win (unrouted) |
| pcbench-mojo-nes_mojo-nes | kicad | java-current | 0.00 | 9 | 0 | 878.4 | | unmeasured | 220.6 | | 17 | 1500.5 | baseline |
| pcbench-mojo-nes_mojo-nes | kicad | rs-main | 0.00 | 7 | 0 | 905.4 | +27.0 | | 68.6 | -152.0 | 13 | 1682.0 | win (unrouted) |
| pcbench-motor-3xdrv8833-hw_ver1 | kicad | java-current | 0.00 | 2 | 137 | 849.2 | | unmeasured | 375.5 | | 101 | 2313.1 | baseline |
| pcbench-motor-3xdrv8833-hw_ver1 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +150.8 | | 188.8 | -186.8 | 88 | 2289.4 | win (clean_pass_rate) |
| pcbench-mppt-2420-hc_mppt-2420-hc | kicad | java-current | 0.00 | 2 | 25 | 977.8 | | unmeasured | 388.1 | | 72 | 3357.0 | baseline |
| pcbench-mppt-2420-hc_mppt-2420-hc | kicad | rs-main | 0.00 | 1 | 5 | 993.6 | +15.9 | | 160.9 | -227.2 | 77 | 3437.3 | win (unrouted) |
| pcbench-mppt-2420-hpx_mppt-2420-hpx | kicad | java-current | 0.00 | 55 | 25 | 851.1 | | unmeasured | 383.8 | | 55 | 3283.6 | baseline |
| pcbench-mppt-2420-hpx_mppt-2420-hpx | kicad | rs-main | 0.00 | 8 | 2 | 979.2 | +128.0 | | 218.0 | -165.8 | 97 | 5145.3 | win (unrouted) |
| pcbench-nRF24breakoutBoard_nRF24-breakout | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 21.7 | | 0 | 101.4 | baseline |
| pcbench-nRF24breakoutBoard_nRF24-breakout | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.2 | -21.5 | 0 | 101.3 | win (score) |
| pcbench-nanoSwinSidC_nanoSwinSidC | kicad | java-current | 0.00 | 3 | 10 | 903.8 | | unmeasured | 225.0 | | 32 | 515.9 | baseline |
| pcbench-nanoSwinSidC_nanoSwinSidC | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +96.2 | | 99.5 | -125.5 | 28 | 522.2 | win (clean_pass_rate) |
| pcbench-navelino-leaf_navelino-leaf | kicad | java-current | 0.00 | 1 | 3 | 944.8 | | unmeasured | 306.7 | | 13 | 280.7 | baseline |
| pcbench-navelino-leaf_navelino-leaf | kicad | rs-main | 0.00 | 1 | 0 | 965.5 | +20.7 | | 41.7 | -265.0 | 12 | 291.2 | win (violations) |
| pcbench-nextbusclock_NextBusClockV1 | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 88.0 | | 4 | 1560.7 | baseline |
| pcbench-nextbusclock_NextBusClockV1 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 10.0 | -78.0 | 2 | 1541.6 | win (score) |
| pcbench-nikon_gps_nikon_gps | kicad | java-current | 0.00 | 2 | 42 | 817.5 | | unmeasured | 92.8 | | 16 | 317.0 | baseline |
| pcbench-nikon_gps_nikon_gps | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +182.5 | | 15.2 | -77.6 | 14 | 295.4 | win (clean_pass_rate) |
| pcbench-nixie-clock_ab18x5-breakout | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 68.4 | | 0 | 121.3 | baseline |
| pcbench-nixie-clock_ab18x5-breakout | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 1.7 | -66.7 | 0 | 121.3 | win (score) |
| pcbench-nodemcu-backstage_NodeMCU Backstage | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 26.6 | | 5 | 359.0 | baseline |
| pcbench-nodemcu-backstage_NodeMCU Backstage | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 4.2 | -22.3 | 2 | 380.8 | win (score) |
| pcbench-nodemcu-basecamp_NodeMCU Basecamp | kicad | java-current | 0.00 | 0 | 9 | 990.5 | | unmeasured | 131.9 | | 28 | 1280.9 | baseline |
| pcbench-nodemcu-basecamp_NodeMCU Basecamp | kicad | rs-main | 0.00 | 0 | 1 | 998.9 | +8.4 | | 113.7 | -18.2 | 20 | 1351.0 | win (violations) |
| pcbench-nrf2rfm69_nrf2rfm69 | kicad | java-current | 0.00 | 1 | 0 | 916.7 | | unmeasured | 52.4 | | 6 | 151.6 | baseline |
| pcbench-nrf2rfm69_nrf2rfm69 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +83.3 | | 0.9 | -51.5 | 4 | 171.6 | win (clean_pass_rate) |
| pcbench-nunchuk_rf_hw_NunchukRF_V3 | kicad | java-current | 0.00 | 5 | 26 | 902.9 | | unmeasured | 353.7 | | 32 | 926.4 | baseline |
| pcbench-nunchuk_rf_hw_NunchukRF_V3 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +97.1 | | 107.5 | -246.2 | 40 | 1063.4 | win (clean_pass_rate) |
| pcbench-oled-bmp280-touch_oled-bmp280-touch | kicad | java-current | 0.00 | 0 | 1 | 990.0 | | unmeasured | 40.2 | | 0 | 545.4 | baseline |
| pcbench-oled-bmp280-touch_oled-bmp280-touch | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +10.0 | | 0.6 | -39.7 | 0 | 537.1 | win (clean_pass_rate) |
| pcbench-one-shift-register_one-shift-register | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 48.9 | | 0 | 377.2 | baseline |
| pcbench-one-shift-register_one-shift-register | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 1.4 | -47.5 | 0 | 385.9 | loss (score) |
| pcbench-onion2-breakout_onion2 breakout | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 58.1 | | 12 | 589.5 | baseline |
| pcbench-onion2-breakout_onion2 breakout | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 10.2 | -47.9 | 6 | 615.0 | win (score) |
| pcbench-opentilt_opentilt2 | kicad | java-current | 0.00 | 1 | 0 | 958.3 | | unmeasured | 40.6 | | 0 | 397.0 | baseline |
| pcbench-opentilt_opentilt2 | kicad | rs-main | 0.00 | 2 | 0 | 916.7 | -41.7 | | 9.8 | -30.8 | 1 | 353.8 | loss (unrouted) |
| pcbench-oshtimer_transponder | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 51.7 | | 1 | 66.8 | baseline |
| pcbench-oshtimer_transponder | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.3 | -51.4 | 1 | 66.7 | win (score) |
| pcbench-ottawa-badges-2016_ottawa-badge-tagger-2016 | kicad | java-current | 0.00 | 0 | 4 | 973.3 | | unmeasured | 30.0 | | 13 | 290.8 | baseline |
| pcbench-ottawa-badges-2016_ottawa-badge-tagger-2016 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +26.7 | | 2.5 | -27.4 | 7 | 285.0 | win (clean_pass_rate) |
| pcbench-ozinverter_ozinverterkicad | kicad | java-current | 0.00 | 1 | 0 | 993.1 | | unmeasured | 99.2 | | 0 | 2138.4 | baseline |
| pcbench-ozinverter_ozinverterkicad | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +6.9 | | 40.4 | -58.9 | 0 | 2099.9 | win (clean_pass_rate) |
| pcbench-pcb-covox-amp-v2_pcb-covox-amp-v2 | kicad | java-current | 0.00 | 1 | 0 | 992.0 | | unmeasured | 132.5 | | 59 | 1486.4 | baseline |
| pcbench-pcb-covox-amp-v2_pcb-covox-amp-v2 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +8.0 | | 29.6 | -103.0 | 28 | 1523.1 | win (clean_pass_rate) |
| pcbench-pcb-covox-amp_pcb-covox-amp | kicad | java-current | 0.00 | 1 | 0 | 987.3 | | unmeasured | 110.8 | | 33 | 820.4 | baseline |
| pcbench-pcb-covox-amp_pcb-covox-amp | kicad | rs-main | 0.00 | 1 | 0 | 987.3 | +0.0 | | 94.9 | -15.9 | 15 | 850.5 | win (score) |
| pcbench-pcb-ks0108-128x64-glcd_circuit | kicad | java-current | 0.00 | 6 | 1 | 870.8 | | unmeasured | 144.5 | | 21 | 345.6 | baseline |
| pcbench-pcb-ks0108-128x64-glcd_circuit | kicad | rs-main | 0.00 | 8 | 1 | 829.2 | -41.7 | | 57.4 | -87.1 | 8 | 398.6 | loss (unrouted) |
| pcbench-pcb-usb-ft245r-parallel-adapter_pcb-usb-ft245r-parallel-adapter | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 135.9 | | 12 | 411.5 | baseline |
| pcbench-pcb-usb-ft245r-parallel-adapter_pcb-usb-ft245r-parallel-adapter | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 7.8 | -128.1 | 8 | 429.5 | win (score) |
| pcbench-pesho_pesho | kicad | java-current | 0.00 | 0 | 2 | 996.6 | | unmeasured | 88.2 | | 5 | 1539.6 | baseline |
| pcbench-pesho_pesho | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +3.4 | | 12.7 | -75.5 | 2 | 1573.3 | win (clean_pass_rate) |
| pcbench-phone_rtty_interface_phone_rtty_rev_a | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 36.5 | | 0 | 443.0 | baseline |
| pcbench-phone_rtty_interface_phone_rtty_rev_a | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 0.8 | -35.7 | 0 | 445.1 | loss (score) |
| pcbench-phone_rtty_interface_phone_rtty_rev_b | kicad | java-current | 0.00 | 1 | 0 | 985.1 | | unmeasured | 46.5 | | 1 | 496.9 | baseline |
| pcbench-phone_rtty_interface_phone_rtty_rev_b | kicad | rs-main | 0.00 | 1 | 0 | 985.1 | +0.0 | | 4.2 | -42.2 | 0 | 490.9 | win (score) |
| pcbench-photon_Sprinkler_sprinkler | kicad | java-current | 0.00 | 0 | 2 | 989.7 | | unmeasured | 30.0 | | 5 | 732.8 | baseline |
| pcbench-photon_Sprinkler_sprinkler | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +10.3 | | 2.0 | -28.0 | 1 | 747.8 | win (clean_pass_rate) |
| pcbench-pi-zero-stepper-board_pi-zero-stepper-board | kicad | java-current | 0.00 | 1 | 0 | 989.6 | | unmeasured | 98.1 | | 5 | 1622.0 | baseline |
| pcbench-pi-zero-stepper-board_pi-zero-stepper-board | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +10.4 | | 14.2 | -83.9 | 2 | 1634.2 | win (clean_pass_rate) |
| pcbench-pi_plant_MCP3002 | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 21.8 | | 0 | 216.9 | baseline |
| pcbench-pi_plant_MCP3002 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 0.2 | -21.5 | 0 | 227.7 | loss (score) |
| pcbench-pico-pi-rel_pico-pi | kicad | java-current | 0.00 | 1 | 89 | 915.7 | | unmeasured | 300.1 | | 58 | 1195.1 | baseline |
| pcbench-pico-pi-rel_pico-pi | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +84.3 | | 144.8 | -155.3 | 36 | 1218.4 | win (clean_pass_rate) |
| pcbench-pocketbone-kicad_pocketbone-kicad | kicad | java-current | 0.00 | 45 | 10 | 759.0 | | unmeasured | 340.2 | | 11 | 793.7 | baseline |
| pcbench-pocketbone-kicad_pocketbone-kicad | kicad | rs-main | 0.00 | 49 | 0 | 748.7 | -10.3 | | 179.6 | -160.7 | 6 | 848.6 | loss (unrouted) |
| pcbench-ponyser-pcb_Ponyser | kicad | java-current | 0.00 | 0 | 1 | 990.0 | | unmeasured | 16.2 | | 2 | 110.9 | baseline |
| pcbench-ponyser-pcb_Ponyser | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +10.0 | | 0.4 | -15.8 | 1 | 101.8 | win (clean_pass_rate) |
| pcbench-prog-cc-100mA_prog-cc-100mA | kicad | java-current | 0.00 | 7 | 0 | 893.9 | | unmeasured | 59.8 | | 0 | 522.1 | baseline |
| pcbench-prog-cc-100mA_prog-cc-100mA | kicad | rs-main | 0.00 | 3 | 0 | 954.5 | +60.6 | | 26.6 | -33.2 | 2 | 596.9 | win (unrouted) |
| pcbench-pulse_v1_pulse | kicad | java-current | 0.00 | 2 | 0 | 928.6 | | unmeasured | 84.2 | | 6 | 142.9 | baseline |
| pcbench-pulse_v1_pulse | kicad | rs-main | 0.00 | 1 | 0 | 964.3 | +35.7 | | 31.1 | -53.1 | 7 | 136.4 | win (unrouted) |
| pcbench-pwm-2420-lus_pwm-2420-lus | kicad | java-current | 0.00 | 16 | 37 | 925.5 | | unmeasured | 336.5 | | 92 | 2712.0 | baseline |
| pcbench-pwm-2420-lus_pwm-2420-lus | kicad | rs-main | 0.00 | 9 | 0 | 971.3 | +45.9 | | 129.1 | -207.4 | 87 | 3070.8 | win (unrouted) |
| pcbench-radio_antenna-iridium | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 169.6 | | 0 | 323.8 | baseline |
| pcbench-radio_antenna-iridium | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 41.9 | -127.8 | 0 | 309.0 | win (score) |
| pcbench-raspberry_pi_pullup_button_pullup_shutdown_button | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 16.3 | | 0 | 52.5 | baseline |
| pcbench-raspberry_pi_pullup_button_pullup_shutdown_button | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.1 | -16.1 | 0 | 52.5 | win (cpu_s) |
| pcbench-raspberry_pi_pullup_button_pullup_shutdown_button(revB) | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 25.9 | | 0 | 70.1 | baseline |
| pcbench-raspberry_pi_pullup_button_pullup_shutdown_button(revB) | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.1 | -25.8 | 0 | 52.3 | win (score) |
| pcbench-rc2014_bank_switcher_z80_cpm_mmu | kicad | java-current | 0.00 | 1 | 0 | 981.8 | | unmeasured | 83.4 | | 1 | 1058.7 | baseline |
| pcbench-rc2014_bank_switcher_z80_cpm_mmu | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +18.2 | | 7.6 | -75.7 | 1 | 1084.4 | win (clean_pass_rate) |
| pcbench-recalbox-gpio-board__autosave-board | kicad | java-current | 0.00 | 1 | 0 | 995.8 | | unmeasured | 337.5 | | 30 | 3903.9 | baseline |
| pcbench-recalbox-gpio-board__autosave-board | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +4.1 | | 68.2 | -269.3 | 18 | 3810.0 | win (clean_pass_rate) |
| pcbench-recalbox-gpio-board_board | kicad | java-current | 0.00 | 1 | 0 | 995.8 | | unmeasured | 338.0 | | 30 | 3903.9 | baseline |
| pcbench-recalbox-gpio-board_board | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +4.1 | | 62.6 | -275.4 | 18 | 3810.0 | win (clean_pass_rate) |
| pcbench-retrocon_bbb-adapter | kicad | java-current | 0.00 | 0 | 5 | 983.0 | | unmeasured | 59.9 | | 3 | 937.7 | baseline |
| pcbench-retrocon_bbb-adapter | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +16.9 | | 4.1 | -55.7 | 2 | 955.9 | win (clean_pass_rate) |
| pcbench-retrocon_driver_board | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 46.1 | | 0 | 874.3 | baseline |
| pcbench-retrocon_driver_board | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 4.7 | -41.4 | 0 | 845.3 | win (score) |
| pcbench-retroreflectors_TANGOFLOCK | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 20.9 | | 0 | 124.8 | baseline |
| pcbench-retroreflectors_TANGOFLOCK | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.3 | -20.7 | 0 | 124.1 | win (score) |
| pcbench-rfcx-sentinel-pcb_Mainboard | kicad | java-current | 0.00 | 0 | 155 | 864.0 | | unmeasured | 240.8 | | 82 | 2069.0 | baseline |
| pcbench-rfcx-sentinel-pcb_Mainboard | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +136.0 | | 167.8 | -73.0 | 41 | 2151.3 | win (clean_pass_rate) |
| pcbench-rfidBoard_rfid | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 53.3 | | 12 | 289.3 | baseline |
| pcbench-rfidBoard_rfid | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 7.3 | -46.1 | 2 | 304.5 | win (score) |
| pcbench-rgb-led_rgb-led-v2 | kicad | java-current | 0.00 | 2 | 0 | 969.2 | | unmeasured | 87.5 | | 2 | 890.2 | baseline |
| pcbench-rgb-led_rgb-led-v2 | kicad | rs-main | 0.00 | 1 | 0 | 984.6 | +15.4 | | 19.2 | -68.3 | 2 | 924.0 | win (unrouted) |
| pcbench-rgb-strip-controller__autosave-rgb-strip | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 34.9 | | 0 | 422.5 | baseline |
| pcbench-rgb-strip-controller__autosave-rgb-strip | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 2.4 | -32.5 | 0 | 400.5 | win (score) |
| pcbench-rgb2ypbpr_rgb2ypbpr | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 278.7 | | 1 | 1364.3 | baseline |
| pcbench-rgb2ypbpr_rgb2ypbpr | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 12.3 | -266.5 | 2 | 1368.0 | loss (score) |
| pcbench-rjw57_cpu-board | kicad | java-current | 0.00 | 75 | 159 | 649.8 | | unmeasured | 362.5 | | 143 | 5294.7 | baseline |
| pcbench-rjw57_cpu-board | kicad | rs-main | 0.00 | 1 | 0 | 996.7 | +346.9 | | 299.0 | -63.5 | 160 | 8469.5 | win (unrouted) |
| pcbench-roomba-ESP12E_roomba-esp | kicad | java-current | 0.00 | 0 | 1 | 995.2 | | unmeasured | 83.6 | | 14 | 668.7 | baseline |
| pcbench-roomba-ESP12E_roomba-esp | kicad | rs-main | 0.00 | 0 | 2 | 990.5 | -4.8 | | 9.8 | -73.8 | 8 | 696.4 | loss (violations) |
| pcbench-rotary-encoder-breakout_rotary-encoder-breakout | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 49.6 | | 2 | 111.8 | baseline |
| pcbench-rotary-encoder-breakout_rotary-encoder-breakout | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 0.6 | -49.0 | 2 | 115.2 | loss (score) |
| pcbench-royer_royer | kicad | java-current | 0.00 | 1 | 0 | 979.2 | | unmeasured | 64.7 | | 7 | 404.7 | baseline |
| pcbench-royer_royer | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +20.8 | | 3.8 | -60.9 | 0 | 408.2 | win (clean_pass_rate) |
| pcbench-rufs__autosave-simple_kicad_schema_and_pcb_v1 | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 14.1 | | 0 | 50.4 | baseline |
| pcbench-rufs__autosave-simple_kicad_schema_and_pcb_v1 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.0 | -14.1 | 0 | 50.4 | win (cpu_s) |
| pcbench-rufs_aprs_tracker | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 216.9 | | 5 | 885.7 | baseline |
| pcbench-rufs_aprs_tracker | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 6.5 | -210.4 | 3 | 894.5 | win (score) |
| pcbench-rufs_dra818v_breakout_board | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 36.4 | | 5 | 277.1 | baseline |
| pcbench-rufs_dra818v_breakout_board | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 2.3 | -34.1 | 5 | 293.2 | loss (score) |
| pcbench-rufs_simple_kicad_schema_and_pcb_v1 | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 13.1 | | 0 | 50.4 | baseline |
| pcbench-rufs_simple_kicad_schema_and_pcb_v1 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | 0.0 | | 0.0 | -13.1 | 0 | 50.4 | win (cpu_s) |
| pcbench-rufs_smart_psu | kicad | java-current | 0.00 | 1 | 20 | 905.7 | | unmeasured | 104.2 | | 15 | 388.6 | baseline |
| pcbench-rufs_smart_psu | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +94.3 | | 4.7 | -99.5 | 3 | 427.8 | win (clean_pass_rate) |
| pcbench-rufs_spv1040_power_controller | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 83.1 | | 2 | 253.2 | baseline |
| pcbench-rufs_spv1040_power_controller | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 2.4 | -80.7 | 1 | 258.8 | win (score) |
| pcbench-rxadc_14_rxadc_14 | kicad | java-current | 0.00 | 0 | 54 | 915.0 | | unmeasured | 167.8 | | 25 | 716.8 | baseline |
| pcbench-rxadc_14_rxadc_14 | kicad | rs-main | 0.00 | 1 | 0 | 992.1 | +77.2 | | 52.7 | -115.2 | 27 | 719.1 | loss (unrouted) |
| pcbench-scimpy_amp | kicad | java-current | 0.00 | 2 | 0 | 977.8 | | unmeasured | 126.4 | | 0 | 1561.0 | baseline |
| pcbench-scimpy_amp | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +22.2 | | 15.9 | -110.5 | 0 | 1530.2 | win (clean_pass_rate) |
| pcbench-scimpy_crossover | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 75.8 | | 0 | 600.2 | baseline |
| pcbench-scimpy_crossover | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 2.8 | -73.0 | 0 | 567.2 | win (score) |
| pcbench-scimpy_powersupply | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 21.1 | | 0 | 233.9 | baseline |
| pcbench-scimpy_powersupply | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 0.6 | -20.6 | 0 | 250.4 | loss (score) |
| pcbench-scimpy_volumebuffer | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 54.3 | | 0 | 717.7 | baseline |
| pcbench-scimpy_volumebuffer | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 9.2 | -45.1 | 0 | 689.3 | win (score) |
| pcbench-sensorboard_DiffIR | kicad | java-current | 0.00 | 1 | 14 | 881.2 | | unmeasured | 63.5 | | 4 | 246.6 | baseline |
| pcbench-sensorboard_DiffIR | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +118.8 | | 3.0 | -60.4 | 3 | 252.5 | win (clean_pass_rate) |
| pcbench-sensorboard_DiffIR.kicad_pcb_narrow | kicad | java-current | 0.00 | 0 | 11 | 931.2 | | unmeasured | 25.0 | | 5 | 255.8 | baseline |
| pcbench-sensorboard_DiffIR.kicad_pcb_narrow | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +68.8 | | 2.2 | -22.7 | 3 | 260.0 | win (clean_pass_rate) |
| pcbench-shutter_Shutter V4 | kicad | java-current | 0.00 | 3 | 0 | 948.3 | | unmeasured | 78.2 | | 2 | 636.2 | baseline |
| pcbench-shutter_Shutter V4 | kicad | rs-main | 0.00 | 6 | 0 | 896.5 | -51.7 | | 20.5 | -57.7 | 0 | 556.2 | loss (unrouted) |
| pcbench-sms-cart-32k_cart | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 79.1 | | 18 | 978.6 | baseline |
| pcbench-sms-cart-32k_cart | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 4.4 | -74.8 | 15 | 991.6 | win (score) |
| pcbench-smt-zvs-driver_IH10-sl | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 67.0 | | 1 | 250.2 | baseline |
| pcbench-smt-zvs-driver_IH10-sl | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 3.9 | -63.0 | 3 | 227.0 | loss (score) |
| pcbench-snappi-zero_snappi-zero | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 116.7 | | 1 | 484.2 | baseline |
| pcbench-snappi-zero_snappi-zero | kicad | rs-main | 0.00 | 0 | 1 | 993.9 | -6.1 | | 4.2 | -112.4 | 2 | 448.6 | loss (clean_pass_rate) |
| pcbench-soil-moisture-sensor-analog_analog-moist-sensor | kicad | java-current | 0.00 | 0 | 34 | 880.7 | | unmeasured | 55.3 | | 16 | 332.5 | baseline |
| pcbench-soil-moisture-sensor-analog_analog-moist-sensor | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +119.3 | | 12.0 | -43.3 | 1 | 346.8 | win (clean_pass_rate) |
| pcbench-solar-lanterns_proto1 | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 32.9 | | 2 | 185.7 | baseline |
| pcbench-solar-lanterns_proto1 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.4 | -32.4 | 1 | 176.0 | win (score) |
| pcbench-sonic3_feram_adapter_sonic3_feram_adapter | kicad | java-current | 0.00 | 0 | 8 | 952.9 | | unmeasured | 103.8 | | 11 | 254.8 | baseline |
| pcbench-sonic3_feram_adapter_sonic3_feram_adapter | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +47.1 | | 5.4 | -98.4 | 9 | 233.6 | win (clean_pass_rate) |
| pcbench-spisolator_spisolator | kicad | java-current | 0.00 | 0 | 4 | 974.2 | | unmeasured | 50.4 | | 11 | 244.5 | baseline |
| pcbench-spisolator_spisolator | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +25.8 | | 25.6 | -24.8 | 6 | 270.1 | win (clean_pass_rate) |
| pcbench-split-pcb-throughole_splanck throughhole | kicad | java-current | 0.00 | 0 | 2 | 994.8 | | unmeasured | 45.5 | | 1 | 1462.9 | baseline |
| pcbench-split-pcb-throughole_splanck throughhole | kicad | rs-main | 0.00 | 0 | 2 | 994.8 | +0.0 | | 2.9 | -42.6 | 0 | 1435.7 | win (score) |
| pcbench-srambo_1_srambo_1 | kicad | java-current | 0.00 | 2 | 114 | 812.1 | | unmeasured | 289.9 | | 109 | 3530.6 | baseline |
| pcbench-srambo_1_srambo_1 | kicad | rs-main | 0.00 | 2 | 0 | 984.8 | +172.7 | | 174.0 | -115.9 | 84 | 3486.3 | win (violations) |
| pcbench-ssr-wifi_adapter | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 43.3 | | 4 | 741.2 | baseline |
| pcbench-ssr-wifi_adapter | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 3.2 | -40.1 | 1 | 701.7 | win (score) |
| pcbench-starfish_starfish | kicad | java-current | 0.00 | 14 | 22 | 835.7 | | unmeasured | 306.8 | | 25 | 613.0 | baseline |
| pcbench-starfish_starfish | kicad | rs-main | 0.00 | 14 | 0 | 875.0 | +39.3 | | 93.7 | -213.2 | 43 | 658.5 | win (violations) |
| pcbench-starsynctrackers_reset_switch | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 12.9 | | 2 | 74.7 | baseline |
| pcbench-starsynctrackers_reset_switch | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.2 | -12.7 | 0 | 79.0 | win (score) |
| pcbench-stlinkv2_breakout_stlink_breakout | kicad | java-current | 0.00 | 1 | 0 | 941.2 | | unmeasured | 19.8 | | 0 | 64.1 | baseline |
| pcbench-stlinkv2_breakout_stlink_breakout | kicad | rs-main | 0.00 | 1 | 0 | 941.2 | +0.0 | | 0.6 | -19.1 | 0 | 62.0 | win (score) |
| pcbench-stm32_ccd_camera_ccd | kicad | java-current | 0.00 | 2 | 84 | 815.7 | | unmeasured | 242.2 | | 53 | 666.1 | baseline |
| pcbench-stm32_ccd_camera_ccd | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +184.3 | | 48.8 | -193.3 | 36 | 626.2 | win (clean_pass_rate) |
| pcbench-stubby_hex | kicad | java-current | 0.00 | 2 | 37 | 942.3 | | unmeasured | 260.3 | | 51 | 1435.7 | baseline |
| pcbench-stubby_hex | kicad | rs-main | 0.00 | 1 | 0 | 993.9 | +51.5 | | 57.1 | -203.2 | 47 | 1530.6 | win (unrouted) |
| pcbench-sv650sds_sds_tool | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 31.1 | | 5 | 304.0 | baseline |
| pcbench-sv650sds_sds_tool | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 1.3 | -29.7 | 1 | 299.7 | win (score) |
| pcbench-tbd_tbd | kicad | java-current | 0.00 | 6 | 16 | 770.0 | | unmeasured | 112.0 | | 12 | 155.9 | baseline |
| pcbench-tbd_tbd | kicad | rs-main | 0.00 | 2 | 0 | 950.0 | +180.0 | | 39.9 | -72.1 | 16 | 163.7 | win (unrouted) |
| pcbench-tdstat_TDstatv2 | kicad | java-current | 0.00 | 0 | 26 | 961.5 | | unmeasured | 139.8 | | 39 | 1852.4 | baseline |
| pcbench-tdstat_TDstatv2 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +38.5 | | 42.6 | -97.1 | 11 | 1948.7 | win (clean_pass_rate) |
| pcbench-technoshield-ui-hw_technoshield | kicad | java-current | 0.00 | 1 | 18 | 963.5 | | unmeasured | 144.8 | | 40 | 2712.0 | baseline |
| pcbench-technoshield-ui-hw_technoshield | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +36.5 | | 52.5 | -92.3 | 19 | 2739.8 | win (clean_pass_rate) |
| pcbench-teensy-touch_teensy-touch | kicad | java-current | 0.00 | 0 | 3 | 966.7 | | unmeasured | 56.9 | | 5 | 557.6 | baseline |
| pcbench-teensy-touch_teensy-touch | kicad | rs-main | 0.00 | 0 | 7 | 922.2 | -44.4 | | 1.5 | -55.4 | 2 | 578.6 | loss (violations) |
| pcbench-teensy-weather-badge_teensyi2c | kicad | java-current | 0.00 | 1 | 0 | 956.5 | | unmeasured | 63.0 | | 1 | 442.1 | baseline |
| pcbench-teensy-weather-badge_teensyi2c | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +43.5 | | 1.5 | -61.5 | 1 | 495.0 | win (clean_pass_rate) |
| pcbench-teensy-wifi-weather-logger_teensyi2c | kicad | java-current | 0.00 | 1 | 0 | 956.5 | | unmeasured | 61.5 | | 1 | 442.1 | baseline |
| pcbench-teensy-wifi-weather-logger_teensyi2c | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +43.5 | | 1.5 | -59.9 | 1 | 495.0 | win (clean_pass_rate) |
| pcbench-temperature-alarm_controller | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 57.0 | | 2 | 860.4 | baseline |
| pcbench-temperature-alarm_controller | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 3.5 | -53.5 | 1 | 816.5 | win (score) |
| pcbench-tepmachcha_tepmachcha | kicad | java-current | 0.00 | 1 | 0 | 916.7 | | unmeasured | 34.4 | | 0 | 185.2 | baseline |
| pcbench-tepmachcha_tepmachcha | kicad | rs-main | 0.00 | 1 | 0 | 916.7 | -0.0 | | 6.0 | -28.4 | 0 | 193.8 | loss (score) |
| pcbench-tessel-ice40__autosave-project | kicad | java-current | 0.00 | 4 | 47 | 834.6 | | unmeasured | 213.0 | | 33 | 695.6 | baseline |
| pcbench-tessel-ice40__autosave-project | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +165.4 | | 94.7 | -118.3 | 24 | 808.5 | win (clean_pass_rate) |
| pcbench-thegrid_thegrid | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 47.7 | | 0 | 1247.1 | baseline |
| pcbench-thegrid_thegrid | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 3.5 | -44.2 | 0 | 1238.3 | win (score) |
| pcbench-thingBot-LoRa_thingBot-LoRa_v1P0 | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 111.7 | | 5 | 341.7 | baseline |
| pcbench-thingBot-LoRa_thingBot-LoRa_v1P0 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 10.0 | -101.7 | 4 | 338.0 | win (score) |
| pcbench-thingBot-LoRa_thingBot-LoRa_v1P1 | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 108.2 | | 4 | 347.1 | baseline |
| pcbench-thingBot-LoRa_thingBot-LoRa_v1P1 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 8.4 | -99.8 | 5 | 346.8 | loss (score) |
| pcbench-timecircuits-hardware_BTTF-TimeCircuits | kicad | java-current | 0.00 | 142 | 2 | 680.7 | | unmeasured | 357.2 | | 143 | 2595.9 | baseline |
| pcbench-timecircuits-hardware_BTTF-TimeCircuits | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +319.3 | | 299.5 | -57.7 | 101 | 5533.8 | win (clean_pass_rate) |
| pcbench-tiny-8088_Computer | kicad | java-current | 0.00 | 11 | 1 | 960.6 | | unmeasured | 356.7 | | 63 | 6579.1 | baseline |
| pcbench-tiny-8088_Computer | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +39.4 | | 194.0 | -162.7 | 53 | 7433.6 | win (clean_pass_rate) |
| pcbench-tinyFISH_tinyBRUSH | kicad | java-current | 0.00 | 1 | 6 | 926.7 | | unmeasured | 52.5 | | 4 | 71.3 | baseline |
| pcbench-tinyFISH_tinyBRUSH | kicad | rs-main | 0.00 | 1 | 0 | 966.7 | +40.0 | | 15.8 | -36.8 | 4 | 69.1 | win (violations) |
| pcbench-tinyisp-micro_tinyispmicro | kicad | java-current | 0.00 | 1 | 2 | 974.1 | | unmeasured | 103.2 | | 19 | 328.0 | baseline |
| pcbench-tinyisp-micro_tinyispmicro | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +25.9 | | 7.7 | -95.6 | 11 | 339.4 | win (clean_pass_rate) |
| pcbench-tinymuseum_museum | kicad | java-current | 0.00 | 2 | 0 | 959.2 | | unmeasured | 111.1 | | 5 | 685.5 | baseline |
| pcbench-tinymuseum_museum | kicad | rs-main | 0.00 | 1 | 0 | 979.6 | +20.4 | | 102.7 | -8.4 | 4 | 731.7 | win (unrouted) |
| pcbench-type5_type5 | kicad | java-current | 0.00 | 0 | 12 | 933.3 | | unmeasured | 115.5 | | 18 | 1846.2 | baseline |
| pcbench-type5_type5 | kicad | rs-main | 0.00 | 0 | 9 | 950.0 | +16.7 | | 22.6 | -92.8 | 10 | 2016.2 | win (violations) |
| pcbench-uC3Moy_uC3Moy | kicad | java-current | 0.00 | 3 | 18 | 839.0 | | unmeasured | 75.6 | | 0 | 302.6 | baseline |
| pcbench-uC3Moy_uC3Moy | kicad | rs-main | 0.00 | 0 | 20 | 902.4 | +63.4 | | 11.9 | -63.7 | 0 | 395.1 | win (unrouted) |
| pcbench-uSKY_uSKY | kicad | java-current | 0.00 | 19 | 29 | 600.0 | | unmeasured | 256.0 | | 0 | 68.7 | baseline |
| pcbench-uSKY_uSKY | kicad | rs-main | 0.00 | 26 | 0 | 580.6 | -19.4 | | 30.4 | -225.7 | 0 | 59.4 | loss (unrouted) |
| pcbench-uext-esp32_UEXT_ESP32 | kicad | java-current | 0.00 | 2 | 5 | 869.6 | | unmeasured | 92.9 | | 10 | 249.9 | baseline |
| pcbench-uext-esp32_UEXT_ESP32 | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +130.4 | | 17.0 | -75.9 | 9 | 303.2 | win (clean_pass_rate) |
| pcbench-usb_rs232c_usb_rs232c_rev_a | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 176.0 | | 16 | 346.7 | baseline |
| pcbench-usb_rs232c_usb_rs232c_rev_a | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 19.8 | -156.2 | 19 | 359.0 | loss (score) |
| pcbench-vatx_vatx | kicad | java-current | 0.00 | 1 | 0 | 984.4 | | unmeasured | 130.4 | | 5 | 667.7 | baseline |
| pcbench-vatx_vatx | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +15.6 | | 13.2 | -117.2 | 9 | 664.8 | win (clean_pass_rate) |
| pcbench-vdcmon_vdcmon | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 75.6 | | 4 | 1209.2 | baseline |
| pcbench-vdcmon_vdcmon | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 7.8 | -67.8 | 2 | 1229.6 | win (score) |
| pcbench-wavegen_rev3 | kicad | java-current | 0.00 | 181 | 0 | 667.3 | | unmeasured | 421.5 | | 18 | 1591.5 | baseline |
| pcbench-wavegen_rev3 | kicad | rs-main | 0.00 | 60 | 0 | 889.7 | +222.4 | | 299.4 | -122.2 | 13 | 2195.2 | win (unrouted) |
| pcbench-wavegen_waveform-generator | kicad | java-current | 0.00 | 10 | 0 | 931.5 | | unmeasured | 358.3 | | 32 | 1092.2 | baseline |
| pcbench-wavegen_waveform-generator | kicad | rs-main | 0.00 | 6 | 0 | 958.9 | +27.4 | | 91.4 | -266.9 | 37 | 1312.9 | win (unrouted) |
| pcbench-wavegen_waveform-generator-rev1 | kicad | java-current | 0.00 | 4 | 0 | 971.6 | | unmeasured | 325.5 | | 60 | 1144.8 | baseline |
| pcbench-wavegen_waveform-generator-rev1 | kicad | rs-main | 0.00 | 3 | 0 | 978.7 | +7.1 | | 85.2 | -240.3 | 54 | 1193.9 | win (unrouted) |
| pcbench-wavegen_wavegen | kicad | java-current | 0.00 | 4 | 0 | 971.6 | | unmeasured | 324.5 | | 60 | 1144.8 | baseline |
| pcbench-wavegen_wavegen | kicad | rs-main | 0.00 | 3 | 0 | 978.7 | +7.1 | | 91.2 | -233.3 | 54 | 1193.9 | win (unrouted) |
| pcbench-wifiLCD_wifilcd | kicad | java-current | 0.00 | 0 | 2 | 992.7 | | unmeasured | 186.5 | | 17 | 630.6 | baseline |
| pcbench-wifiLCD_wifilcd | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +7.3 | | 12.5 | -174.0 | 13 | 629.5 | win (clean_pass_rate) |
| pcbench-xmasOrn_xmasOrn | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 368.1 | | 53 | 2058.9 | baseline |
| pcbench-xmasOrn_xmasOrn | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 299.4 | -68.7 | 54 | 2095.9 | loss (score) |
| pcbench-xwhatits-capsense-controller_model-f-3178-adaptor | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 63.9 | | 0 | 227.6 | baseline |
| pcbench-xwhatits-capsense-controller_model-f-3178-adaptor | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 1.0 | -62.9 | 0 | 230.6 | loss (score) |
| pcbench-z2amiller_sensorboard_programmer | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 23.1 | | 1 | 149.5 | baseline |
| pcbench-z2amiller_sensorboard_programmer | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | +0.0 | | 0.6 | -22.6 | 0 | 156.6 | win (score) |
| pcbench-zx-sizif-128_sizif128 | kicad | java-current | 0.00 | 9 | 0 | 972.4 | | unmeasured | 469.4 | | 57 | 7277.5 | baseline |
| pcbench-zx-sizif-128_sizif128 | kicad | rs-main | 0.00 | 1 | 0 | 996.9 | +24.5 | | 193.5 | -275.9 | 78 | 8361.0 | win (unrouted) |
| pcbench-zx-sizif-512-ext_sizif512ext | kicad | java-current | 0.00 | 127 | 52 | 724.1 | | unmeasured | 479.6 | | 170 | 6460.8 | baseline |
| pcbench-zx-sizif-512-ext_sizif512ext | kicad | rs-main | 0.00 | 63 | 0 | 873.5 | +149.4 | | 299.4 | -180.2 | 225 | 9078.6 | win (unrouted) |
| pcbench-zx-sizif-512-wifi_sizif512-wifi | kicad | java-current | 1.00 | 0 | 0 | 1000.0 | | unmeasured | 100.2 | | 5 | 261.1 | baseline |
| pcbench-zx-sizif-512-wifi_sizif512-wifi | kicad | rs-main | 1.00 | 0 | 0 | 1000.0 | -0.0 | | 4.5 | -95.7 | 6 | 255.5 | loss (score) |
| pcbench-zx-sizif-xxs_sizif-xxs | kicad | java-current | 0.00 | 41 | 57 | 813.5 | | unmeasured | 495.9 | | 172 | 2394.2 | baseline |
| pcbench-zx-sizif-xxs_sizif-xxs | kicad | rs-main | 0.00 | 114 | 0 | 594.3 | -219.2 | | 299.2 | -196.7 | 183 | 1314.3 | loss (unrouted) |

Note: seeds=1 (<3), so the noise floor could not be measured (stddev needs ≥ 3 samples) and is reported as "unmeasured" above; verdicts on this report may be less reliable than one run with ≥ 3 seeds.

## Self-report disagreements

| board | candidate | seed | self unrouted | referee unrouted | self viol | referee viol |
|---|---|---|---|---|---|---|
| pcbench-16x12-bits-I2C_I2C_Servo | java-current | 1 | 0 | 0 | 0 | 53 |
| pcbench-1Bitsy_1bitsy | java-current | 1 | 0 | 0 | 45 | 55 |
| pcbench-6N137-TTL-Serial-Optoisolator_6N137-TTL-Serial-Optoisolator | java-current | 1 | 0 | 0 | 4 | 0 |
| pcbench-6volt-5W-solar-cc_6vleadacidsolar | java-current | 1 | 0 | 0 | 0 | 12 |
| pcbench-74Logic_SA_ADC_SA-ADC | java-current | 1 | 74 | 74 | 32 | 0 |
| pcbench-96boards-sensors_Sensors | java-current | 1 | 14 | 14 | 0 | 8 |
| pcbench-96boards-sensors_Sensors | rs-main | 1 | 2 | 2 | 0 | 9 |
| pcbench-ABOVISP_ABOVISP | java-current | 1 | 1 | 1 | 60 | 7 |
| pcbench-APM-RPi-Shield_APM-RPi-Shield | java-current | 1 | 4 | 4 | 4 | 2 |
| pcbench-APM-RPi-Shield_APM-RPi-Shield | rs-main | 1 | 7 | 7 | 0 | 1 |
| pcbench-AS5043-Encoder_sensor-board | java-current | 1 | 0 | 0 | 0 | 2 |
| pcbench-ATtiny461Breakout_ATTiny461DevBoard | java-current | 1 | 1 | 1 | 0 | 2 |
| pcbench-ATtiny461Breakout_ATTiny461DevBoard | rs-main | 1 | 0 | 0 | 0 | 1 |
| pcbench-AVR-ISP_level-shifter_AVR-ISP_level-shifter | java-current | 1 | 1 | 1 | 0 | 4 |
| pcbench-AVR-ISP_pogo-plug_1.27mm_AVR-ISP_pogo-plug_1.27mm | java-current | 1 | 0 | 0 | 14 | 0 |
| pcbench-Amiga-A1012-PCB_Amiga-A1012 | java-current | 1 | 0 | 0 | 0 | 42 |
| pcbench-AnalogThermometer_AnalogThermometer | java-current | 1 | 0 | 0 | 0 | 3 |
| pcbench-Apple-M0110-BT_Apple M0110 | java-current | 1 | 0 | 0 | 8 | 0 |
| pcbench-Avem_Hardware_Avem_demo | java-current | 1 | 1 | 1 | 0 | 39 |
| pcbench-AzizLight_AzizLight | java-current | 1 | 0 | 0 | 0 | 23 |
| pcbench-BB-PWR-3608_BB-PWR-3608_revA | java-current | 1 | 0 | 0 | 17 | 0 |
| pcbench-BB-PWR-8009_BB-PWR-8009_revA | java-current | 1 | 0 | 0 | 18 | 0 |
| pcbench-BLDC-controller_BLDC_controller | java-current | 1 | 135 | 134 | 1007 | 46 |
| pcbench-BLDC-controller_BLDC_controller | rs-main | 1 | 20 | 20 | 0 | 1 |
| pcbench-BirdAttractor_BirdAttractor_RevA | java-current | 1 | 0 | 0 | 2 | 0 |
| pcbench-BirthdayCakeKeyboard_10Key | java-current | 1 | 0 | 0 | 0 | 62 |
| pcbench-Biscay_Blueeye_mcu | java-current | 1 | 0 | 0 | 0 | 17 |
| pcbench-Biscay_Blueeye_sipm-fpga | java-current | 1 | 2 | 2 | 0 | 77 |
| pcbench-Box0-hv-analog-breakoutboard_breakout | java-current | 1 | 134 | 128 | 12 | 0 |
| pcbench-Box0-hv-analog-breakoutboard_breakout | rs-main | 1 | 134 | 128 | 0 | 0 |
| pcbench-Brushless_ESC_Brushless_ESC | java-current | 1 | 1 | 1 | 0 | 69 |
| pcbench-C-BISCUIT_crowbar | java-current | 1 | 0 | 0 | 0 | 3 |
| pcbench-CAL430FR_CAL430F | java-current | 1 | 8 | 8 | 0 | 28 |
| pcbench-CAL430FR_CAL430F_watch | java-current | 1 | 0 | 0 | 0 | 18 |
| pcbench-CapPCB_CapPcb | java-current | 1 | 0 | 0 | 8 | 0 |
| pcbench-ChirpHardware_chirp | java-current | 1 | 1 | 1 | 6 | 65 |
| pcbench-CompactFlashBreakout_CompactFlashBreakout | java-current | 1 | 0 | 0 | 0 | 1 |
| pcbench-CoreOne-xCORE200-Original_CoreOne | java-current | 1 | 309 | 302 | 33 | 3 |
| pcbench-CoreOne-xCORE200-Original_CoreOne | rs-main | 1 | 21 | 21 | 0 | 1 |
| pcbench-Curryboard_Curryboard | java-current | 1 | 3 | 3 | 56 | 0 |
| pcbench-Curryboard_Curryboard | rs-main | 1 | 0 | 0 | 0 | 2 |
| pcbench-DAC-ADAU1966_DAC-ADAU1966 | java-current | 1 | 103 | 102 | 0 | 0 |
| pcbench-DC25_DC25 | java-current | 1 | 0 | 0 | 0 | 25 |
| pcbench-DC25_DC25 | rs-main | 1 | 0 | 0 | 0 | 9 |
| pcbench-DIYDAC_DIYDAC | java-current | 1 | 0 | 0 | 1 | 0 |
| pcbench-DPS-1200FB_Adapter_Adapter | java-current | 1 | 1 | 1 | 0 | 3 |
| pcbench-DPS-1200FB_Adapter_Adapter | rs-main | 1 | 1 | 1 | 0 | 2 |
| pcbench-DaWeather---Project__autosave-CarteDaWeather | rs-main | 1 | 0 | 0 | 0 | 1 |
| pcbench-DasBlinkinput_Das Blinkinput | java-current | 1 | 0 | 0 | 8 | 0 |
| pcbench-Dekada_dekada | java-current | 1 | 0 | 0 | 8 | 0 |
| pcbench-Dekada_dekada_TopoR_curves | java-current | 1 | 0 | 0 | 8 | 0 |
| pcbench-DerKnopf_digi-pot | java-current | 1 | 1 | 1 | 6 | 54 |
| pcbench-DerKnopf_led-ring | java-current | 1 | 0 | 0 | 0 | 4 |
| pcbench-DerKnopf_power-supply | java-current | 1 | 0 | 0 | 4 | 0 |
| pcbench-DiscoDanceFloorV1_DiscoDongle | java-current | 1 | 0 | 0 | 6 | 11 |
| pcbench-DoroidOscillo-Board_Android_Oscilloscope | java-current | 1 | 13 | 13 | 42 | 67 |
| pcbench-DualLM317BenchSupply_DualLM317BenchSupply | java-current | 1 | 0 | 0 | 8 | 0 |
| pcbench-DustSensorShield_DustSensorShield | java-current | 1 | 1 | 1 | 6 | 0 |
| pcbench-ESP32-Module-Breakout_ESP32S-breakout | java-current | 1 | 0 | 0 | 0 | 13 |
| pcbench-ESP8266-MQTT-battery-monitor-hw_battery-monitor | java-current | 1 | 0 | 0 | 0 | 27 |
| pcbench-ESPLux_Board | java-current | 1 | 0 | 0 | 8 | 0 |
| pcbench-ESP_BaPoTeSta_ESP_BaPoTeSta | java-current | 1 | 2 | 2 | 2 | 0 |
| pcbench-ESP_WiFiSwitch_WifiSwitch | java-current | 1 | 0 | 0 | 0 | 1 |
| pcbench-Electronics-MainBoard_MainBoard | java-current | 1 | 1 | 1 | 6 | 42 |
| pcbench-EncoderBoard_Enc_Pan_Led | java-current | 1 | 7 | 7 | 8 | 26 |
| pcbench-EncoderBoard_Enc_Pan_Led | rs-main | 1 | 3 | 3 | 2 | 4 |
| pcbench-FRM16_Relay_Module_I2C_Controller_relay_controller | java-current | 1 | 0 | 0 | 36 | 3 |
| pcbench-FT231X_breakout_FTDI_FT231XS-U_Breakout | java-current | 1 | 3 | 3 | 0 | 3 |
| pcbench-FlashProgrammer_flash_programmer | java-current | 1 | 5 | 5 | 0 | 52 |
| pcbench-FogDrive_attiny45_slim | java-current | 1 | 0 | 0 | 2 | 0 |
| pcbench-HW-AC-Emeter_ac-power-monitor | java-current | 1 | 1 | 1 | 0 | 13 |
| pcbench-Hangul-Clock_Hangul | java-current | 1 | 0 | 0 | 0 | 28 |
| pcbench-Hardware-done-with-kicad_AVRlearn | java-current | 1 | 0 | 0 | 0 | 3 |
| pcbench-Hardware_Playground_BL_PCB_latest | java-current | 1 | 0 | 0 | 0 | 24 |
| pcbench-Hardware_Playground_Touch_Switch_1ch_PCB | java-current | 1 | 1 | 1 | 0 | 19 |
| pcbench-Hardware_Playground_buck_led_driver | java-current | 1 | 1 | 1 | 14 | 0 |
| pcbench-Hardware_Playground_esp8266_uno_relay | java-current | 1 | 1 | 1 | 56 | 0 |
| pcbench-Hardware_Playground_hy_adapter | java-current | 1 | 0 | 0 | 16 | 0 |
| pcbench-Hardware_Playground_minimal_node_rfm69w | java-current | 1 | 9 | 9 | 0 | 8 |
| pcbench-Hardware_Playground_minimal_node_rfm69w | rs-main | 1 | 6 | 6 | 0 | 1 |
| pcbench-Hardware_Playground_nrf52832_uno | java-current | 1 | 4 | 4 | 0 | 36 |
| pcbench-Hardware_Playground_orange_pi_zero_node | java-current | 1 | 1 | 1 | 0 | 2 |
| pcbench-Hardware_Playground_orange_pi_zero_node | rs-main | 1 | 1 | 1 | 0 | 1 |
| pcbench-Hardware_Playground_rpi_zero | java-current | 1 | 2 | 2 | 0 | 21 |
| pcbench-Hardware_Playground_rpi_zero | rs-main | 1 | 0 | 0 | 0 | 11 |
| pcbench-Hardware_Playground_serial_gw_ATMEGA328P | java-current | 1 | 1 | 1 | 14 | 57 |
| pcbench-Hardware_Playground_serial_gw_ATMEGA328P | rs-main | 1 | 1 | 1 | 0 | 16 |
| pcbench-HaveSome_PCB_HaveSomePCB | java-current | 1 | 1 | 1 | 10 | 0 |
| pcbench-HellScribe_HellScribe | java-current | 1 | 0 | 0 | 0 | 25 |
| pcbench-Inhibition_amplifier | java-current | 1 | 0 | 0 | 8 | 0 |
| pcbench-Inkjet_InkjetBreakout | java-current | 1 | 0 | 0 | 0 | 21 |
| pcbench-Inkjet_InkjetDriver | java-current | 1 | 0 | 0 | 0 | 6 |
| pcbench-Inkjet_PiezoDriver | java-current | 1 | 1 | 1 | 6 | 9 |
| pcbench-Inkjet__autosave-InkjetDriver | java-current | 1 | 0 | 0 | 0 | 6 |
| pcbench-JLink-SWD_JLink-SWD | java-current | 1 | 0 | 0 | 1 | 0 |
| pcbench-Kefersender_UKW TX | java-current | 1 | 7 | 6 | 10 | 8 |
| pcbench-Kefersender_UKW TX | rs-main | 1 | 6 | 5 | 0 | 0 |
| pcbench-Keyboard_PCB_Keyboard | java-current | 1 | 260 | 260 | 2 | 37 |
| pcbench-KiCad-LTC6802-2_main | java-current | 1 | 0 | 0 | 0 | 47 |
| pcbench-KiCad-LTC6802-2_main | rs-main | 1 | 0 | 0 | 0 | 2 |
| pcbench-LAUNCHXL-F28027-isolation-PCB_project1 | java-current | 1 | 1 | 1 | 8 | 4 |
| pcbench-LED-Square_PT4115_LED-Square_PT4115 | java-current | 1 | 0 | 0 | 42 | 6 |
| pcbench-LPC2148_Stick_LPC2148_stick | java-current | 1 | 33 | 33 | 0 | 42 |
| pcbench-LPC2148_Stick_LPC2148_stick | rs-main | 1 | 2 | 2 | 0 | 2 |
| pcbench-LPC2148_Stick__autosave-LPC2148_stick | java-current | 1 | 35 | 35 | 0 | 39 |
| pcbench-LPC2148_Stick__autosave-LPC2148_stick | rs-main | 1 | 2 | 2 | 0 | 2 |
| pcbench-LT3652EvalBoard_LT3652EvalBoard | java-current | 1 | 0 | 0 | 0 | 30 |
| pcbench-LVDS2TMDS_LVDS2TMDS | java-current | 1 | 0 | 0 | 0 | 32 |
| pcbench-LadyBugShield_LBS-TEST1 | java-current | 1 | 0 | 0 | 1 | 8 |
| pcbench-LiFePO4-Charge-Controller_LiFePO4-Charge-Controller | java-current | 1 | 0 | 0 | 0 | 20 |
| pcbench-Librecalc-Hardware__autosave-calculator | java-current | 1 | 173 | 173 | 5 | 0 |
| pcbench-LoRaCatTrack_GPSLoRa | java-current | 1 | 4 | 4 | 40 | 0 |
| pcbench-LoRaPP_loramod | java-current | 1 | 0 | 0 | 73 | 31 |
| pcbench-LongPixel_AnalogDriverMini | java-current | 1 | 0 | 0 | 2 | 15 |
| pcbench-MAGFest-2017-Swadges_magfest_badges | java-current | 1 | 0 | 0 | 20 | 0 |
| pcbench-MAVRIC_Hardware_ArduinoPracticeBoard | java-current | 1 | 0 | 0 | 0 | 20 |
| pcbench-MAVRIC_Hardware_Motherboard | java-current | 1 | 0 | 0 | 0 | 29 |
| pcbench-MAVRIC_Hardware_SoilBoard | java-current | 1 | 0 | 0 | 0 | 30 |
| pcbench-MAVRIC_Hardware__autosave-Motherboard | java-current | 1 | 0 | 0 | 0 | 29 |
| pcbench-Mechaduino-DR_Mechaduino DR 1.01 | java-current | 1 | 1 | 1 | 20 | 76 |
| pcbench-Minitel_bbb-adapter | java-current | 1 | 0 | 0 | 0 | 5 |
| pcbench-MySRaspiGW_MySRaspiGW | java-current | 1 | 3 | 3 | 3 | 0 |
| pcbench-MySRaspiGW_MySRaspiGW_PA_LNA | java-current | 1 | 2 | 2 | 3 | 0 |
| pcbench-NavigationThing_NavigationThing | java-current | 1 | 1 | 1 | 1 | 0 |
| pcbench-NavigationThing_NavigationThing | rs-main | 1 | 0 | 0 | 0 | 3 |
| pcbench-NeoWall_NeoWall | java-current | 1 | 0 | 0 | 0 | 8 |
| pcbench-Omega2-Berrydock_berrydock-mini | java-current | 1 | 1 | 1 | 6 | 81 |
| pcbench-Omega2-Berrydock_berrydock-mini | rs-main | 1 | 1 | 1 | 1 | 12 |
| pcbench-OpAmpPassXsistorBenchSupply_OpAmpPassXsistorBenchSupply | java-current | 1 | 3 | 3 | 0 | 3 |
| pcbench-Open-Source-Power-Supply_PowerSupply_PCB | java-current | 1 | 0 | 0 | 0 | 3 |
| pcbench-Open-Source-Power-Supply_PowerSupply_PCB_backup | java-current | 1 | 0 | 0 | 0 | 3 |
| pcbench-OpenHardwareExG_ActiveElectrode_OpenHardwareExG_ActiveElectrode | java-current | 1 | 3 | 3 | 0 | 3 |
| pcbench-OpenHardwareExG_Shield_OpenHardwareExG_Shield | java-current | 1 | 45 | 45 | 0 | 119 |
| pcbench-OpenHardwareExG_Shield_OpenHardwareExG_Shield | rs-main | 1 | 1 | 1 | 0 | 42 |
| pcbench-OpenHardwareExG_Shield_OpenHardwareExG_Shield_Test_Board | java-current | 1 | 3 | 3 | 0 | 64 |
| pcbench-OpenHardwareExG_Shield_OpenHardwareExG_Shield_Test_Board | rs-main | 1 | 0 | 0 | 0 | 2 |
| pcbench-OpenHardwareExG_Shield_OpenHardwareExG_Shield_Test_Board_all_panelled | java-current | 1 | 3 | 3 | 0 | 66 |
| pcbench-OpenHardwareExG_Shield_OpenHardwareExG_Shield_Test_Board_all_panelled | rs-main | 1 | 0 | 0 | 0 | 2 |
| pcbench-OpenVNAVI_driver unit | java-current | 1 | 4 | 4 | 0 | 23 |
| pcbench-Own-Mailbox-Hardware_eth | java-current | 1 | 35 | 35 | 10 | 0 |
| pcbench-Own-Mailbox-Hardware_mailbox | java-current | 1 | 42 | 42 | 10 | 0 |
| pcbench-PCB_constant_current_ac_hv | java-current | 1 | 0 | 0 | 0 | 5 |
| pcbench-PCB_constant_current_ac_hv | rs-main | 1 | 0 | 0 | 0 | 8 |
| pcbench-PGA2311_pga2311 | java-current | 1 | 0 | 0 | 9 | 0 |
| pcbench-PixyWirelessShield_Shield PIXY | java-current | 1 | 0 | 0 | 0 | 2 |
| pcbench-PocketBone_pocketbone-kicad | java-current | 1 | 48 | 48 | 2 | 9 |
| pcbench-Practicas-Curso-Kicad_Ejercicio_2 | java-current | 1 | 0 | 0 | 0 | 37 |
| pcbench-Practicas-Curso-Kicad_Salguero_Federico2 | java-current | 1 | 0 | 0 | 0 | 30 |
| pcbench-Practicas-Curso-Kicad_Salguero_Federico2 | rs-main | 1 | 0 | 0 | 2 | 19 |
| pcbench-QRPCard_QRPCard | java-current | 1 | 0 | 0 | 0 | 38 |
| pcbench-R1002_R1002 | java-current | 1 | 1 | 1 | 49 | 31 |
| pcbench-R1002_R1002 | rs-main | 1 | 9 | 3 | 0 | 0 |
| pcbench-RC2014_RC2014 IDE | java-current | 1 | 0 | 0 | 78 | 0 |
| pcbench-RC2014_RC2014 RAM | java-current | 1 | 0 | 0 | 92 | 0 |
| pcbench-RC2014_RC2014 Tandy Sound Card | java-current | 1 | 2 | 2 | 92 | 0 |
| pcbench-RFM69HCW_ATSHA204A_Breakout_RFM69HCW_ATSHA204A_Breakout | java-current | 1 | 1 | 1 | 2 | 17 |
| pcbench-RGBMatrixPanelCPLD-PhotonBackpack_RGBMatrixPanel_CPLD_negative | java-current | 1 | 0 | 0 | 56 | 0 |
| pcbench-RGBMatrixPanelCPLD-PhotonBackpack_RGBMatrixPanel_CPLD_positive | java-current | 1 | 0 | 0 | 56 | 0 |
| pcbench-RX5808_diversityModule | java-current | 1 | 0 | 0 | 18 | 0 |
| pcbench-RX5808_rx5808_4button | java-current | 1 | 2 | 1 | 11 | 0 |
| pcbench-RX5808_rx5808_4button | rs-main | 1 | 1 | 0 | 0 | 0 |
| pcbench-Raspberry-Pi-Soft-Power-Controller_Switching Supply TPS563208 MCI | java-current | 1 | 0 | 0 | 0 | 6 |
| pcbench-ReST32_ReST SD-Module | java-current | 1 | 0 | 0 | 1 | 0 |
| pcbench-RoBoC_RoboticsMKII | java-current | 1 | 43 | 43 | 2 | 47 |
| pcbench-RoBoC_RoboticsMKII | rs-main | 1 | 0 | 0 | 0 | 14 |
| pcbench-S1G-Mod_JST_Adapter | java-current | 1 | 0 | 0 | 2 | 0 |
| pcbench-SMDBreakouts_smd_breakout | java-current | 1 | 0 | 0 | 0 | 72 |
| pcbench-SMDBreakouts_smd_breakout_quad | java-current | 1 | 0 | 0 | 0 | 72 |
| pcbench-SNAP-Badge_SNAP_badge | java-current | 1 | 61 | 61 | 0 | 1 |
| pcbench-STM32F303_LQFP48_STM32_LQFP48 | java-current | 1 | 0 | 0 | 80 | 2 |
| pcbench-STM32F373_LQFP48_STM32_LQFP48 | java-current | 1 | 0 | 0 | 90 | 0 |
| pcbench-SimpleCPLD_SimpleCPLD | java-current | 1 | 2 | 2 | 0 | 13 |
| pcbench-Solare-BQ24210_Solare-BQ24210 | java-current | 1 | 0 | 0 | 0 | 8 |
| pcbench-Starburst-One_alpha | java-current | 1 | 0 | 0 | 36 | 0 |
| pcbench-Starling_Starling_V1 | java-current | 1 | 1 | 1 | 0 | 19 |
| pcbench-TLPHnodeV2_TLPHnodeV2 | java-current | 1 | 1 | 1 | 7 | 19 |
| pcbench-TMC261-stepstick_TMC261-stepstick-v1.1 | java-current | 1 | 17 | 17 | 0 | 54 |
| pcbench-TX5823_TX5823 | java-current | 1 | 13 | 12 | 368 | 2 |
| pcbench-TX5823_TX5823 | rs-main | 1 | 7 | 6 | 0 | 2 |
| pcbench-Teensy-3.5-Breakout-Boaard_PropShield_Uno | java-current | 1 | 1 | 1 | 6 | 0 |
| pcbench-Teensy-Hats_Teensy-7-Segment-Hat | java-current | 1 | 0 | 0 | 30 | 0 |
| pcbench-ThinkerShield_ThinkerShield | java-current | 1 | 0 | 0 | 24 | 0 |
| pcbench-TinyTracker_ub-minimal | java-current | 1 | 4 | 4 | 22 | 70 |
| pcbench-ToslinkCNC__autosave-ToslinkCNC_OneAxis | java-current | 1 | 8 | 8 | 0 | 19 |
| pcbench-ULPI-Pmod_ULPI-Pmod | java-current | 1 | 2 | 2 | 6 | 16 |
| pcbench-UProgrammer-Hardware_Programmer | java-current | 1 | 18 | 18 | 30 | 21 |
| pcbench-UltrasonicSystem_Schematic | java-current | 1 | 0 | 0 | 2 | 16 |
| pcbench-Usb-Serial-Breakout-Cp2102_cp2102 | java-current | 1 | 3 | 3 | 26 | 10 |
| pcbench-VC4000MultiROM_MultiRomCard | java-current | 1 | 3 | 3 | 0 | 10 |
| pcbench-VC4000MultiROM_MultiRomCard | rs-main | 1 | 6 | 6 | 0 | 6 |
| pcbench-WHCS-Base-Station_base-station | java-current | 1 | 0 | 0 | 0 | 15 |
| pcbench-WHCS-Base-Station_base-station | rs-main | 1 | 0 | 0 | 0 | 1 |
| pcbench-WS2811LEDMatrix_matrixcontrol | java-current | 1 | 0 | 0 | 26 | 27 |
| pcbench-WeatherSpot_vreg_pressure | java-current | 1 | 0 | 0 | 0 | 4 |
| pcbench-analog_esr_meter_esr_meter_rev_a | java-current | 1 | 0 | 0 | 4 | 0 |
| pcbench-arduino-led-driver_arduino-led-driver | java-current | 1 | 10 | 10 | 0 | 114 |
| pcbench-atmegax8-protoboard_atmegax8-protoboard | java-current | 1 | 0 | 0 | 56 | 0 |
| pcbench-audio_relay_input_switch_relay_switch | java-current | 1 | 0 | 0 | 9 | 3 |
| pcbench-autohat-board_usd-adapter | java-current | 1 | 2 | 2 | 0 | 3 |
| pcbench-avr-fuser-32_adapter | java-current | 1 | 8 | 8 | 0 | 8 |
| pcbench-avr_ledprojector_avr_ledprojection | java-current | 1 | 2 | 2 | 0 | 26 |
| pcbench-avr_ledprojector_avr_ledprojection-0402 | java-current | 1 | 2 | 2 | 2 | 60 |
| pcbench-badge2016_Badge_init | java-current | 1 | 0 | 0 | 17 | 0 |
| pcbench-balena-rover-wide-hat_resin-rover | java-current | 1 | 0 | 0 | 6 | 57 |
| pcbench-balena-rover-wide-hat_resin-rover | rs-main | 1 | 0 | 0 | 0 | 4 |
| pcbench-basic_esp_board_basic_esp_board | java-current | 1 | 1 | 1 | 0 | 4 |
| pcbench-beast-phat_beast-phat | java-current | 1 | 0 | 0 | 0 | 5 |
| pcbench-beer-gauge_sensorboard | java-current | 1 | 0 | 0 | 12 | 1 |
| pcbench-beryl_rain_beryl_rain | java-current | 1 | 0 | 0 | 0 | 1 |
| pcbench-bikedar_bikedar | java-current | 1 | 0 | 0 | 6 | 61 |
| pcbench-bikedar_bikedar | rs-main | 1 | 0 | 0 | 0 | 3 |
| pcbench-blackmagic-isolated_mmp | java-current | 1 | 1 | 1 | 7 | 28 |
| pcbench-bldc-gimbal-1d_gimbal-board | java-current | 1 | 0 | 0 | 0 | 38 |
| pcbench-blinky-badge_blinky | java-current | 1 | 0 | 0 | 162 | 0 |
| pcbench-bms-8s50-ic_bms-8s50-ic | java-current | 1 | 170 | 168 | 128 | 41 |
| pcbench-bms-8s50-ic_bms-8s50-ic | rs-main | 1 | 54 | 52 | 0 | 0 |
| pcbench-bmw-ibus-bluetooth_bmw_bt_cdcemu_analog | java-current | 1 | 1 | 1 | 4 | 2 |
| pcbench-bmw-ibus-bluetooth_bmw_bt_cdcemu_digital | java-current | 1 | 0 | 0 | 4 | 0 |
| pcbench-boatcontrol_CommonCathode60A | java-current | 1 | 98 | 98 | 256 | 0 |
| pcbench-boatcontrol_CommonCathode60A | rs-main | 1 | 97 | 95 | 0 | 0 |
| pcbench-bobc_LCD-panel-adapter-lvc | java-current | 1 | 2 | 2 | 0 | 20 |
| pcbench-bobc_MS-F100 | java-current | 1 | 14 | 14 | 5 | 37 |
| pcbench-bobc_led_clock | java-current | 1 | 0 | 0 | 2 | 17 |
| pcbench-bobc_matrix_clock | java-current | 1 | 37 | 37 | 0 | 1 |
| pcbench-bpnode-bb_BPnode-BB | java-current | 1 | 1 | 1 | 2 | 3 |
| pcbench-breakout-boards_avr-isp-x2 | java-current | 1 | 0 | 0 | 16 | 0 |
| pcbench-breakout-boards_esp8266-jtag | java-current | 1 | 0 | 0 | 0 | 50 |
| pcbench-bypass_crossmix_bypass_crossmix | java-current | 1 | 12 | 12 | 0 | 14 |
| pcbench-can_firewall_hardware_CAN_Firewall | java-current | 1 | 2 | 2 | 18 | 91 |
| pcbench-cdm324_backpack_cdm324 | java-current | 1 | 0 | 0 | 16 | 0 |
| pcbench-ciurlys_ciurlys | java-current | 1 | 7 | 7 | 4 | 0 |
| pcbench-cnlohr_wiflier | java-current | 1 | 38 | 38 | 16 | 2 |
| pcbench-cnlohr_wiflier | rs-main | 1 | 41 | 41 | 3 | 0 |
| pcbench-cnlohr_wiflier_B | java-current | 1 | 35 | 35 | 19 | 0 |
| pcbench-cnlohr_wiflier_B | rs-main | 1 | 41 | 41 | 3 | 0 |
| pcbench-continuity-tester_continuity-tester | java-current | 1 | 1 | 1 | 0 | 11 |
| pcbench-continuity-tester_continuity-tester | rs-main | 1 | 1 | 1 | 0 | 4 |
| pcbench-custom_cpu--ALU_custom_cpu--ALU | java-current | 1 | 0 | 0 | 4 | 0 |
| pcbench-custom_cpu--register_custom_cpu--register | java-current | 1 | 0 | 0 | 4 | 0 |
| pcbench-data-manager_data-manager | java-current | 1 | 1 | 1 | 8 | 31 |
| pcbench-decelerator4030_decelerator4030 | java-current | 1 | 782 | 499 | 0 | 205 |
| pcbench-decelerator4030_decelerator4030 | rs-main | 1 | 488 | 488 | 7 | 5 |
| pcbench-denbit_basic | java-current | 1 | 0 | 0 | 12 | 0 |
| pcbench-devttys0_IRis | java-current | 1 | 0 | 0 | 0 | 3 |
| pcbench-digital_clock_led_clock_3_and_4_digit | java-current | 1 | 0 | 0 | 0 | 3 |
| pcbench-digital_clock_led_clock_v1 | java-current | 1 | 1 | 1 | 0 | 13 |
| pcbench-digital_clock_led_clock_v1 | rs-main | 1 | 0 | 0 | 0 | 3 |
| pcbench-disco-dongle_DiscoDongle | java-current | 1 | 0 | 0 | 4 | 35 |
| pcbench-divergence_meter_dm_control | java-current | 1 | 8 | 8 | 24 | 59 |
| pcbench-domotics_base-board-arranged | java-current | 1 | 20 | 20 | 2 | 3 |
| pcbench-domotics_base-board-arranged | rs-main | 1 | 0 | 0 | 5 | 13 |
| pcbench-dorkyboard_keyboard | java-current | 1 | 421 | 421 | 0 | 107 |
| pcbench-dorkyboard_keyboard | rs-main | 1 | 3 | 3 | 0 | 16 |
| pcbench-dust_sensor_dust_sensor | java-current | 1 | 1 | 1 | 0 | 29 |
| pcbench-eeg_brainboard_batteryv0 | java-current | 1 | 10 | 5 | 10 | 18 |
| pcbench-eeg_brainboard_batteryv0 | rs-main | 1 | 5 | 1 | 0 | 0 |
| pcbench-eink-adapter_eink | java-current | 1 | 35 | 35 | 16 | 0 |
| pcbench-epapercard_epapercard | java-current | 1 | 6 | 6 | 0 | 2 |
| pcbench-esp-serial-terminal_esp-com | java-current | 1 | 0 | 0 | 0 | 18 |
| pcbench-esp12-appliance_mod_esp12-appliance-mod | java-current | 1 | 0 | 0 | 0 | 13 |
| pcbench-esp12-breakout_ESP12Breakout | java-current | 1 | 0 | 0 | 0 | 3 |
| pcbench-esp32-ethernet_esp32-ethernet | java-current | 1 | 2 | 2 | 4 | 1 |
| pcbench-esp32-ethernet_esp32-ethernet | rs-main | 1 | 0 | 0 | 0 | 4 |
| pcbench-esp32stack_esp32stack | java-current | 1 | 0 | 0 | 6 | 0 |
| pcbench-esp8266_32x32panel_esp_12_f_595 | java-current | 1 | 1 | 1 | 8 | 0 |
| pcbench-esp8266_32x32panel_esp_12_f_595_ORDERED | java-current | 1 | 1 | 1 | 8 | 0 |
| pcbench-esp8266_envmonitor_environment-monitor | java-current | 1 | 2 | 2 | 0 | 4 |
| pcbench-esp8266_envmonitor_environment-monitor-1.2 | java-current | 1 | 0 | 0 | 0 | 2 |
| pcbench-esp8266_envmonitor_environment-monitor-1.4 | java-current | 1 | 2 | 2 | 0 | 4 |
| pcbench-esp8266_link_test_esp_micro85-only | java-current | 1 | 5 | 5 | 5 | 21 |
| pcbench-esp8266_network_speaker_esp_network_speaker | java-current | 1 | 0 | 0 | 10 | 0 |
| pcbench-esp8266_wi07_3_adapter_esp | java-current | 1 | 0 | 0 | 0 | 13 |
| pcbench-espalarm_alarm | java-current | 1 | 0 | 0 | 0 | 85 |
| pcbench-espalarm_alarm | rs-main | 1 | 0 | 0 | 0 | 64 |
| pcbench-espeverywhere__autosave-espeverywhere_breakout | java-current | 1 | 0 | 0 | 0 | 18 |
| pcbench-espionage_esplight | java-current | 1 | 0 | 0 | 1 | 7 |
| pcbench-everled_everled | java-current | 1 | 0 | 0 | 6 | 0 |
| pcbench-fan_controller_fan_controller | java-current | 1 | 0 | 0 | 0 | 21 |
| pcbench-fifogfx_c64cart | java-current | 1 | 3 | 3 | 44 | 0 |
| pcbench-filament_extruder_sensor | java-current | 1 | 0 | 0 | 0 | 12 |
| pcbench-fp2_extension_sample_fp2_usb_breakout | java-current | 1 | 1 | 1 | 2 | 6 |
| pcbench-free-of-charge_BMS | java-current | 1 | 36 | 36 | 0 | 116 |
| pcbench-freeDSP-CLASSIC-SMD-BALANCED_FreeDSP_BAL | java-current | 1 | 152 | 152 | 2 | 125 |
| pcbench-guitar_fret | java-current | 1 | 2 | 2 | 0 | 32 |
| pcbench-hardware-designs_nixie-power | java-current | 1 | 0 | 0 | 0 | 5 |
| pcbench-headstage-adapter_headstage adapter | java-current | 1 | 13 | 13 | 22 | 74 |
| pcbench-helmholtz-servo_CurrentServo | java-current | 1 | 1 | 1 | 4 | 22 |
| pcbench-hm-mod-rpi-rtc_hm-mod-rpi-rtc | java-current | 1 | 0 | 0 | 0 | 3 |
| pcbench-hw_trials_demo | java-current | 1 | 1 | 1 | 2 | 6 |
| pcbench-hwstar_ac-power-monitor | java-current | 1 | 1 | 1 | 0 | 13 |
| pcbench-icehat_icehat | java-current | 1 | 16 | 16 | 42 | 2 |
| pcbench-induction-hob_temperature-sender | java-current | 1 | 3 | 3 | 0 | 12 |
| pcbench-jadonk_PocketBone | java-current | 1 | 45 | 45 | 2 | 8 |
| pcbench-jdy-08-board_jdy-08 | java-current | 1 | 1 | 1 | 0 | 35 |
| pcbench-juno-chorus-clone_juno-chorus-clone | java-current | 1 | 1 | 1 | 2 | 3 |
| pcbench-juno-chorus-clone_juno-chorus-clone | rs-main | 1 | 5 | 5 | 0 | 4 |
| pcbench-karabas-nano_karabas-nano-revA | java-current | 1 | 272 | 272 | 0 | 159 |
| pcbench-karabas-nano_karabas-nano-revB | java-current | 1 | 275 | 275 | 0 | 199 |
| pcbench-karabas-nano_karabas-nano-revC | java-current | 1 | 334 | 334 | 33 | 1 |
| pcbench-karabas-nano_karabas-nano-revC | rs-main | 1 | 96 | 96 | 0 | 1 |
| pcbench-karabas-nano_karabas-nano-revG | java-current | 1 | 323 | 323 | 10 | 0 |
| pcbench-karabas-nano_wifi_revA | java-current | 1 | 0 | 0 | 0 | 9 |
| pcbench-kassenautomat.mdb-interface_mdb-interface | java-current | 1 | 0 | 0 | 0 | 68 |
| pcbench-kicad-projects_BatCharge | java-current | 1 | 0 | 0 | 0 | 4 |
| pcbench-kika-in-space_DS8500 | java-current | 1 | 0 | 0 | 0 | 10 |
| pcbench-kika-in-space_analog-test-board | java-current | 1 | 0 | 0 | 0 | 19 |
| pcbench-kit2-led-cube_led_cube | java-current | 1 | 0 | 0 | 68 | 7 |
| pcbench-kitspace_40-channel-hv-switching-board | java-current | 1 | 172 | 172 | 0 | 22 |
| pcbench-kitspace_BQ25570_Harvester | java-current | 1 | 6 | 6 | 40 | 2 |
| pcbench-kitspace_CH330 | java-current | 1 | 2 | 2 | 12 | 3 |
| pcbench-kitspace_CO2 | java-current | 1 | 0 | 0 | 2 | 18 |
| pcbench-kitspace_DIY_detector | java-current | 1 | 0 | 0 | 10 | 4 |
| pcbench-kitspace_DIY_detector | rs-main | 1 | 0 | 0 | 0 | 1 |
| pcbench-kitspace_OSO-BOOK-C1 | java-current | 1 | 20 | 5 | 113 | 34 |
| pcbench-kitspace_OSO-BOOK-C1 | rs-main | 1 | 56 | 7 | 0 | 31 |
| pcbench-kitspace_OtterScreen | java-current | 1 | 0 | 0 | 0 | 17 |
| pcbench-kitspace_PSLab | java-current | 1 | 13 | 13 | 8 | 86 |
| pcbench-kitspace_T32_ref | java-current | 1 | 0 | 0 | 0 | 15 |
| pcbench-kitspace_USB-C-Screen-Adapter | java-current | 1 | 13 | 12 | 14 | 10 |
| pcbench-kitspace_USB-C-Screen-Adapter | rs-main | 1 | 4 | 4 | 0 | 3 |
| pcbench-kitspace_USB-C-Screen-Adapter-LDR6023SS | java-current | 1 | 0 | 0 | 6 | 0 |
| pcbench-kitspace__autosave-nunchuk_breakout | java-current | 1 | 0 | 0 | 2 | 0 |
| pcbench-kitspace_ardfpga | java-current | 1 | 20 | 25 | 0 | 54 |
| pcbench-kitspace_ardfpga | rs-main | 1 | 5 | 10 | 0 | 0 |
| pcbench-kitspace_dropbot-front-panel | java-current | 1 | 67 | 67 | 0 | 6 |
| pcbench-kitspace_dropbot_control_board | java-current | 1 | 4 | 6 | 5 | 59 |
| pcbench-kitspace_dropbot_control_board | rs-main | 1 | 0 | 2 | 0 | 5 |
| pcbench-kitspace_dynamixel_shield | java-current | 1 | 5 | 5 | 0 | 8 |
| pcbench-kitspace_esp8266 | java-current | 1 | 0 | 0 | 2 | 0 |
| pcbench-kitspace_ideal_diode | java-current | 1 | 0 | 0 | 18 | 0 |
| pcbench-kitspace_led_driver | java-current | 1 | 0 | 0 | 4 | 0 |
| pcbench-kitspace_level_shifter | java-current | 1 | 0 | 0 | 2 | 0 |
| pcbench-kitspace_minisumo_v3 | rs-main | 1 | 2 | 2 | 0 | 3 |
| pcbench-kitspace_nunchuk_breakout | java-current | 1 | 0 | 0 | 3 | 0 |
| pcbench-kitspace_pmt_combiner | java-current | 1 | 0 | 0 | 0 | 13 |
| pcbench-kitspace_pmt_combiner | rs-main | 1 | 0 | 0 | 0 | 1 |
| pcbench-kitspace_sensor | java-current | 1 | 2 | 2 | 4 | 128 |
| pcbench-kitspace_sympetrum-v2%20NFF1.1 | java-current | 1 | 14 | 14 | 0 | 55 |
| pcbench-kitspace_teensy-fx | java-current | 1 | 29 | 29 | 0 | 54 |
| pcbench-kitspace_threeboard | java-current | 1 | 0 | 0 | 0 | 23 |
| pcbench-led-wordclock_wordclock | java-current | 1 | 7 | 7 | 1 | 63 |
| pcbench-led-wordclock_wordclock | rs-main | 1 | 5 | 5 | 0 | 4 |
| pcbench-light-painting-wand_light-wand | java-current | 1 | 0 | 0 | 1 | 42 |
| pcbench-linklayer_contact | java-current | 1 | 1 | 1 | 0 | 32 |
| pcbench-low-power-counter_lpcounter | java-current | 1 | 0 | 0 | 4 | 0 |
| pcbench-m2-electronics_m2fc | java-current | 1 | 131 | 131 | 18 | 201 |
| pcbench-m2-electronics_m2fc | rs-main | 1 | 34 | 34 | 0 | 4 |
| pcbench-m2-electronics_m2pogo | java-current | 1 | 0 | 0 | 0 | 4 |
| pcbench-m2-electronics_m2pogo | rs-main | 1 | 0 | 0 | 0 | 4 |
| pcbench-m2-electronics_m2r | java-current | 1 | 1 | 1 | 10 | 40 |
| pcbench-m2-electronics_m2rl | java-current | 1 | 0 | 0 | 1 | 2 |
| pcbench-mac-pro-conversion_front-panel-power-adapter | java-current | 1 | 0 | 0 | 48 | 0 |
| pcbench-mdbwerk_mdbwerk | java-current | 1 | 5 | 5 | 13 | 6 |
| pcbench-mdbwerk_mdbwerk | rs-main | 1 | 3 | 3 | 0 | 2 |
| pcbench-medusa_medusa_rs422_rx | java-current | 1 | 0 | 0 | 0 | 35 |
| pcbench-mightyduino_mightyduino | java-current | 1 | 6 | 6 | 84 | 0 |
| pcbench-mini_ice40_mini_ice40 | java-current | 1 | 0 | 0 | 0 | 80 |
| pcbench-mini_ice40_mini_ice40 | rs-main | 1 | 0 | 0 | 0 | 15 |
| pcbench-miniboard-opamp_miniboard-opamp | java-current | 1 | 0 | 0 | 20 | 2 |
| pcbench-miniboard-stm32f0_miniboard-stm32f0 | java-current | 1 | 0 | 0 | 8 | 9 |
| pcbench-mobile-sensor-pcb_mobile-sensor-pcb | java-current | 1 | 2 | 2 | 10 | 6 |
| pcbench-mobile-sensor-pcb_mobile-sensor-pcb | rs-main | 1 | 1 | 1 | 0 | 4 |
| pcbench-motor-3xdrv8833-hw_ver1 | java-current | 1 | 2 | 2 | 0 | 137 |
| pcbench-mppt-2420-hc_mppt-2420-hc | java-current | 1 | 2 | 2 | 1 | 25 |
| pcbench-mppt-2420-hc_mppt-2420-hc | rs-main | 1 | 1 | 1 | 0 | 5 |
| pcbench-mppt-2420-hpx_mppt-2420-hpx | java-current | 1 | 55 | 55 | 17 | 25 |
| pcbench-mppt-2420-hpx_mppt-2420-hpx | rs-main | 1 | 8 | 8 | 0 | 2 |
| pcbench-nanoSwinSidC_nanoSwinSidC | java-current | 1 | 3 | 3 | 56 | 10 |
| pcbench-navelino-leaf_navelino-leaf | java-current | 1 | 1 | 1 | 1 | 3 |
| pcbench-nikon_gps_nikon_gps | java-current | 1 | 2 | 2 | 3 | 42 |
| pcbench-nixie-clock_ab18x5-breakout | java-current | 1 | 0 | 0 | 1 | 0 |
| pcbench-nodemcu-basecamp_NodeMCU Basecamp | java-current | 1 | 0 | 0 | 0 | 9 |
| pcbench-nodemcu-basecamp_NodeMCU Basecamp | rs-main | 1 | 0 | 0 | 0 | 1 |
| pcbench-nunchuk_rf_hw_NunchukRF_V3 | java-current | 1 | 5 | 5 | 8 | 26 |
| pcbench-oled-bmp280-touch_oled-bmp280-touch | java-current | 1 | 0 | 0 | 0 | 1 |
| pcbench-one-shift-register_one-shift-register | java-current | 1 | 0 | 0 | 8 | 0 |
| pcbench-oshtimer_transponder | java-current | 1 | 0 | 0 | 4 | 0 |
| pcbench-ottawa-badges-2016_ottawa-badge-tagger-2016 | java-current | 1 | 0 | 0 | 0 | 4 |
| pcbench-pcb-ks0108-128x64-glcd_circuit | java-current | 1 | 5 | 6 | 42 | 1 |
| pcbench-pcb-ks0108-128x64-glcd_circuit | rs-main | 1 | 7 | 8 | 0 | 1 |
| pcbench-pcb-usb-ft245r-parallel-adapter_pcb-usb-ft245r-parallel-adapter | java-current | 1 | 0 | 0 | 4 | 0 |
| pcbench-pesho_pesho | java-current | 1 | 0 | 0 | 0 | 2 |
| pcbench-photon_Sprinkler_sprinkler | java-current | 1 | 0 | 0 | 0 | 2 |
| pcbench-pico-pi-rel_pico-pi | java-current | 1 | 1 | 1 | 14 | 89 |
| pcbench-pocketbone-kicad_pocketbone-kicad | java-current | 1 | 45 | 45 | 2 | 10 |
| pcbench-ponyser-pcb_Ponyser | java-current | 1 | 0 | 0 | 0 | 1 |
| pcbench-pulse_v1_pulse | java-current | 1 | 2 | 2 | 6 | 0 |
| pcbench-pwm-2420-lus_pwm-2420-lus | java-current | 1 | 16 | 16 | 79 | 37 |
| pcbench-radio_antenna-iridium | java-current | 1 | 0 | 0 | 14 | 0 |
| pcbench-raspberry_pi_pullup_button_pullup_shutdown_button | java-current | 1 | 0 | 0 | 11 | 0 |
| pcbench-raspberry_pi_pullup_button_pullup_shutdown_button(revB) | java-current | 1 | 0 | 0 | 8 | 0 |
| pcbench-retrocon_bbb-adapter | java-current | 1 | 0 | 0 | 0 | 5 |
| pcbench-retroreflectors_TANGOFLOCK | java-current | 1 | 0 | 0 | 4 | 0 |
| pcbench-rfcx-sentinel-pcb_Mainboard | java-current | 1 | 0 | 0 | 0 | 155 |
| pcbench-rgb2ypbpr_rgb2ypbpr | java-current | 1 | 0 | 0 | 6 | 0 |
| pcbench-rjw57_cpu-board | java-current | 1 | 75 | 75 | 0 | 159 |
| pcbench-roomba-ESP12E_roomba-esp | java-current | 1 | 0 | 0 | 2 | 1 |
| pcbench-roomba-ESP12E_roomba-esp | rs-main | 1 | 0 | 0 | 0 | 2 |
| pcbench-rotary-encoder-breakout_rotary-encoder-breakout | java-current | 1 | 0 | 0 | 18 | 0 |
| pcbench-royer_royer | java-current | 1 | 1 | 1 | 2 | 0 |
| pcbench-rufs_aprs_tracker | java-current | 1 | 0 | 0 | 6 | 0 |
| pcbench-rufs_smart_psu | java-current | 1 | 1 | 1 | 0 | 20 |
| pcbench-rufs_spv1040_power_controller | java-current | 1 | 0 | 0 | 7 | 0 |
| pcbench-rxadc_14_rxadc_14 | java-current | 1 | 0 | 0 | 6 | 54 |
| pcbench-sensorboard_DiffIR | java-current | 1 | 1 | 1 | 0 | 14 |
| pcbench-sensorboard_DiffIR.kicad_pcb_narrow | java-current | 1 | 0 | 0 | 0 | 11 |
| pcbench-shutter_Shutter V4 | java-current | 1 | 3 | 3 | 4 | 0 |
| pcbench-smt-zvs-driver_IH10-sl | java-current | 1 | 0 | 0 | 30 | 0 |
| pcbench-snappi-zero_snappi-zero | java-current | 1 | 0 | 0 | 8 | 0 |
| pcbench-snappi-zero_snappi-zero | rs-main | 1 | 0 | 0 | 0 | 1 |
| pcbench-soil-moisture-sensor-analog_analog-moist-sensor | java-current | 1 | 0 | 0 | 0 | 34 |
| pcbench-solar-lanterns_proto1 | java-current | 1 | 0 | 0 | 20 | 0 |
| pcbench-sonic3_feram_adapter_sonic3_feram_adapter | java-current | 1 | 0 | 0 | 48 | 8 |
| pcbench-spisolator_spisolator | java-current | 1 | 0 | 0 | 0 | 4 |
| pcbench-split-pcb-throughole_splanck throughhole | java-current | 1 | 0 | 0 | 0 | 2 |
| pcbench-split-pcb-throughole_splanck throughhole | rs-main | 1 | 0 | 0 | 0 | 2 |
| pcbench-srambo_1_srambo_1 | java-current | 1 | 2 | 2 | 0 | 114 |
| pcbench-starfish_starfish | java-current | 1 | 15 | 14 | 0 | 22 |
| pcbench-stm32_ccd_camera_ccd | java-current | 1 | 2 | 2 | 0 | 84 |
| pcbench-stubby_hex | java-current | 1 | 2 | 2 | 9 | 37 |
| pcbench-tbd_tbd | java-current | 1 | 6 | 6 | 6 | 16 |
| pcbench-tdstat_TDstatv2 | java-current | 1 | 0 | 0 | 0 | 26 |
| pcbench-technoshield-ui-hw_technoshield | java-current | 1 | 1 | 1 | 0 | 18 |
| pcbench-teensy-touch_teensy-touch | java-current | 1 | 0 | 0 | 0 | 3 |
| pcbench-teensy-touch_teensy-touch | rs-main | 1 | 0 | 0 | 0 | 7 |
| pcbench-tepmachcha_tepmachcha | java-current | 1 | 1 | 1 | 2 | 0 |
| pcbench-tessel-ice40__autosave-project | java-current | 1 | 4 | 4 | 0 | 47 |
| pcbench-thingBot-LoRa_thingBot-LoRa_v1P0 | java-current | 1 | 0 | 0 | 46 | 0 |
| pcbench-thingBot-LoRa_thingBot-LoRa_v1P1 | java-current | 1 | 0 | 0 | 46 | 0 |
| pcbench-timecircuits-hardware_BTTF-TimeCircuits | java-current | 1 | 142 | 142 | 0 | 2 |
| pcbench-tiny-8088_Computer | java-current | 1 | 11 | 11 | 0 | 1 |
| pcbench-tinyFISH_tinyBRUSH | java-current | 1 | 1 | 1 | 38 | 6 |
| pcbench-tinyisp-micro_tinyispmicro | java-current | 1 | 1 | 1 | 3 | 2 |
| pcbench-type5_type5 | java-current | 1 | 0 | 0 | 0 | 12 |
| pcbench-type5_type5 | rs-main | 1 | 0 | 0 | 0 | 9 |
| pcbench-uC3Moy_uC3Moy | java-current | 1 | 3 | 3 | 10 | 18 |
| pcbench-uC3Moy_uC3Moy | rs-main | 1 | 0 | 0 | 0 | 20 |
| pcbench-uSKY_uSKY | java-current | 1 | 19 | 19 | 28 | 29 |
| pcbench-uSKY_uSKY | rs-main | 1 | 27 | 26 | 0 | 0 |
| pcbench-uext-esp32_UEXT_ESP32 | java-current | 1 | 2 | 2 | 2 | 5 |
| pcbench-usb_rs232c_usb_rs232c_rev_a | java-current | 1 | 0 | 0 | 9 | 0 |
| pcbench-vatx_vatx | java-current | 1 | 1 | 1 | 8 | 0 |
| pcbench-wavegen_rev3 | java-current | 1 | 181 | 181 | 228 | 0 |
| pcbench-wavegen_waveform-generator-rev1 | java-current | 1 | 4 | 4 | 1 | 0 |
| pcbench-wavegen_wavegen | java-current | 1 | 4 | 4 | 1 | 0 |
| pcbench-wifiLCD_wifilcd | java-current | 1 | 0 | 0 | 4 | 2 |
| pcbench-xmasOrn_xmasOrn | java-current | 1 | 0 | 0 | 5 | 0 |
| pcbench-xwhatits-capsense-controller_model-f-3178-adaptor | java-current | 1 | 0 | 0 | 60 | 0 |
| pcbench-zx-sizif-128_sizif128 | java-current | 1 | 9 | 9 | 6 | 0 |
| pcbench-zx-sizif-512-ext_sizif512ext | java-current | 1 | 127 | 127 | 94 | 52 |
| pcbench-zx-sizif-512-wifi_sizif512-wifi | java-current | 1 | 0 | 0 | 3 | 0 |
| pcbench-zx-sizif-xxs_sizif-xxs | java-current | 1 | 41 | 41 | 43 | 57 |

## Failures

- pcbench-STM32F373_LQFP48_STM32_LQFP48 / rs-main / seed 1: out.ses missing (candidate produced no output)
