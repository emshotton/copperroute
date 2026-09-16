# Full target-search experiment — 2026-09-13

Base: `66612acf`. 751 eligible boards (740 PCBench, eleven local fixtures), all scored by KiCad 10.0.4. Ten passes, 300-second deadline, one router thread, twelve jobs on workbench. This evaluates all eligible KiCad boards in the imported workbench corpus; Java-only fixtures are excluded.

Positive deltas mean worse. Copper/routing and solder-mask findings are shown separately; both matter. CPU ratios include different routing work and deadline effects. Some boards reach the mask report cap; uncapped pairs are reported separately.

| Variant / subset | Paired | Unrouted baseline → candidate (Δ) | U improved / regressed boards | Copper Δ / gaining boards | Mask Δ / gaining boards | CPU ratio | Fully connected baseline → candidate | Deadline baseline → candidate | Max RSS baseline → candidate (MiB) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| targets / all | 751 | 4584 → 4864 (+280) | 36 / 52 | +14 / 28 | -28 / 33 | 1.065× | 602 → 601 | 27 → 36 | 295.2 → 254.6 |
| targets / pcbench | 740 | 4354 → 4632 (+278) | 35 / 51 | +14 / 28 | -28 / 33 | 1.069× | 595 → 593 | 26 → 35 | 295.2 → 254.6 |
| targets / local | 11 | 230 → 232 (+2) | 1 / 1 | +0 / 0 | +0 / 0 | 0.928× | 7 → 8 | 1 → 1 | 135.2 → 117.7 |

## Deadline and report-cap controls

- targets: both sides completed normally on 714 boards: unrouted Δ +17, copper Δ -14, mask Δ -4. Mask counts below the cap on both sides: 724 boards, Δ -29, 20 gaining findings.

## Per-board changes

| Variant | Board | Unrouted | Copper | Mask | Either side reached deadline? |
|---|---|---:|---:|---:|---|
| targets | pcbench-kitspace_USB-C-Screen-Adapter | 3 → 4 | 2 → 2 | 83 → 83 | False |
| targets | pcbench-1-Wire-Wing-pcb_1-Wire_Wing | 0 → 0 | 0 → 0 | 26 → 28 | False |
| targets | pcbench-1Bitsy_1bitsy | 0 → 3 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-655_testboard | 0 → 0 | 0 → 0 | 91 → 99 | False |
| targets | pcbench-8bit-cpu_programming_interface | 5 → 3 | 0 → 0 | 1 → 32 | False |
| targets | pcbench-96boards-sensors_Sensors | 0 → 1 | 21 → 8 | 1 → 2 | False |
| targets | pcbench-AIOsense_AIOsense | 18 → 19 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-AS5043-Encoder_sensor-board | 0 → 0 | 0 → 0 | 30 → 34 | False |
| targets | pcbench-AmpOne_dev-AmpOne | 0 → 0 | 0 → 3 | 0 → 0 | False |
| targets | pcbench-Apple-M0110-BT_Apple M0110 | 0 → 0 | 0 → 0 | 79 → 78 | False |
| targets | pcbench-BLDC-controller_BLDC_controller | 11 → 14 | 9 → 15 | 215 → 223 | False |
| targets | pcbench-Blitz_.C68 | 499 → 435 | 0 → 0 | 0 → 0 | True |
| targets | pcbench-Blitz_copy | 483 → 499 | 0 → 0 | 0 → 0 | True |
| targets | pcbench-CAL430FR_CAL430F | 0 → 5 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-CATs-Eurosynth_Abakus_Main | 15 → 18 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-CATs-Eurosynth_CGS_Analog_Switch_Matrix_Main | 8 → 9 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-CATs-Eurosynth_Classic_ADSR_Main | 3 → 5 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-CATs-Eurosynth_HAGIWO_MultiOut_Main | 7 → 9 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-CATs-Eurosynth_HAGIWO_Ring_Modulator_Main | 4 → 6 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-CATs-Eurosynth_Slim_Precision_Adder_Main | 4 → 3 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-CATs-Eurosynth_Slimline_Voltage_Controlled_Switch_-_Main | 5 → 9 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-CATs-Eurosynth_YuSynth_Improved_Steiner_VCF_Main | 3 → 4 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-CoreOne-xCORE200-Original_CoreOne | 1 → 2 | 1 → 1 | 0 → 0 | True |
| targets | pcbench-Curryboard_Curryboard | 0 → 0 | 0 → 5 | 0 → 0 | True |
| targets | pcbench-DAC-ADAU1966_DAC-ADAU1966 | 0 → 0 | 0 → 0 | 124 → 136 | False |
| targets | pcbench-DC25_DC25 | 0 → 0 | 11 → 8 | 29 → 29 | False |
| targets | pcbench-DoroidOscillo-Board_Android_Oscilloscope | 5 → 2 | 0 → 0 | 48 → 56 | False |
| targets | pcbench-EEGFrontier_EEGFrontier | 0 → 1 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-ESP07-Breakout_ESP07-Breakout | 0 → 0 | 2 → 0 | 0 → 0 | False |
| targets | pcbench-Electronics-MainBoard_MainBoard | 0 → 0 | 0 → 0 | 201 → 202 | False |
| targets | pcbench-EuroPi_europi-surface-mount | 27 → 32 | 18 → 17 | 0 → 0 | False |
| targets | pcbench-FT231X_breakout_FTDI_FT231XS-U_Breakout | 0 → 0 | 0 → 0 | 66 → 69 | False |
| targets | pcbench-Feather-ICE40-PCB_feather_ice40 | 26 → 27 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-GameTiger_GameTiger | 25 → 20 | 199 → 199 | 0 → 0 | False |
| targets | pcbench-Hardware_Playground_minimal_node_rfm69w | 3 → 3 | 4 → 0 | 0 → 0 | False |
| targets | pcbench-Hardware_Playground_rpi_zero | 0 → 0 | 31 → 20 | 0 → 0 | False |
| targets | pcbench-Hardware_Playground_serial_gw_ATMEGA328P | 1 → 1 | 0 → 14 | 140 → 140 | False |
| targets | pcbench-HaveSome_PCB_HaveSomePCB | 0 → 0 | 0 → 0 | 5 → 6 | False |
| targets | pcbench-Hubble_12_bit_analog_out | 5 → 4 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-Hubble_16_bit_analog_out | 7 → 5 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-Hubble_leds | 4 → 5 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-Hubble_mux | 1 → 2 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-Inkjet_PiezoDriver | 0 → 1 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-LED-Square_PT4115_LED-Square_PT4115 | 0 → 0 | 2 → 0 | 0 → 0 | False |
| targets | pcbench-LPC2148_Stick_LPC2148_stick | 3 → 2 | 3 → 0 | 182 → 197 | False |
| targets | pcbench-LPC2148_Stick__autosave-LPC2148_stick | 3 → 2 | 3 → 0 | 182 → 197 | True |
| targets | pcbench-Librecalc-Hardware__autosave-calculator | 1 → 4 | 0 → 0 | 200 → 201 | True |
| targets | pcbench-LoRaPP_loramod | 0 → 0 | 0 → 0 | 174 → 179 | False |
| targets | pcbench-Mechaduino-DR_Mechaduino DR 1.01 | 0 → 0 | 0 → 0 | 203 → 202 | False |
| targets | pcbench-MixSID_mixsid | 0 → 0 | 0 → 0 | 202 → 203 | False |
| targets | pcbench-NRC2016_banked_ram | 0 → 0 | 0 → 0 | 207 → 202 | False |
| targets | pcbench-NRC2016_usb_sio | 0 → 0 | 0 → 0 | 166 → 138 | False |
| targets | pcbench-NavigationThing_NavigationThing | 0 → 0 | 3 → 0 | 0 → 0 | False |
| targets | pcbench-OpenHardwareExG_Shield_OpenHardwareExG_Shield | 3 → 2 | 39 → 37 | 8 → 6 | False |
| targets | pcbench-OpenHardwareExG_Shield_OpenHardwareExG_Shield_Test_Board | 0 → 0 | 2 → 0 | 0 → 0 | False |
| targets | pcbench-OpenHardwareExG_Shield_OpenHardwareExG_Shield_Test_Board_all_panelled | 0 → 0 | 2 → 0 | 0 → 0 | False |
| targets | pcbench-Own-Mailbox-Hardware_eth | 1 → 3 | 0 → 0 | 204 → 206 | True |
| targets | pcbench-Own-Mailbox-Hardware_mailbox | 2 → 7 | 0 → 0 | 203 → 207 | True |
| targets | pcbench-Pi1541io_Pi1541io | 9 → 6 | 12 → 23 | 0 → 0 | False |
| targets | pcbench-Pi5_PCIe_Pi5_PCIe | 8 → 14 | 9 → 10 | 0 → 0 | False |
| targets | pcbench-PmodHDMIIn_PmodHDMIIn | 0 → 0 | 0 → 0 | 149 → 134 | False |
| targets | pcbench-RC6502-Apple-1-Replica_RC6502_Apple_1_SBC | 16 → 18 | 0 → 0 | 0 → 0 | True |
| targets | pcbench-ReSDMAC_ReSDMAC | 121 → 182 | 0 → 0 | 0 → 0 | True |
| targets | pcbench-RetroWiFiModem_RetroWiFiModem | 0 → 0 | 1 → 2 | 0 → 0 | False |
| targets | pcbench-RoBoC_RoboticsMKII | 0 → 0 | 3 → 9 | 0 → 0 | True |
| targets | pcbench-SNAP-Badge_SNAP_badge | 0 → 0 | 2 → 0 | 2 → 0 | True |
| targets | pcbench-STM32F303_LQFP48_STM32_LQFP48 | 1 → 0 | 0 → 0 | 114 → 100 | False |
| targets | pcbench-STM32F373_LQFP48_STM32_LQFP48 | 0 → 0 | 0 → 0 | 104 → 119 | False |
| targets | pcbench-Solare-BQ24210_Solare-BQ24210 | 1 → 1 | 0 → 0 | 14 → 12 | False |
| targets | pcbench-TB6600StepperDriver_DEW_TB6600-V1 | 6 → 4 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-TLPHnodeV2_TLPHnodeV2 | 0 → 0 | 0 → 0 | 120 → 116 | False |
| targets | pcbench-TX5823_TX5823 | 3 → 2 | 2 → 0 | 128 → 128 | False |
| targets | pcbench-Teensy-3.5-Breakout-Boaard_TeensyMegaIoShield | 2 → 0 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-TinyTracker_ub-minimal | 3 → 3 | 0 → 0 | 201 → 199 | False |
| targets | pcbench-ULPI-Pmod_ULPI-Pmod | 0 → 1 | 0 → 0 | 75 → 75 | False |
| targets | pcbench-UProgrammer-Hardware_Programmer | 2 → 1 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-VC4000MultiROM_MultiRomCard | 2 → 1 | 13 → 18 | 0 → 0 | False |
| targets | pcbench-WHCS-Base-Station_base-station | 0 → 0 | 1 → 3 | 0 → 0 | False |
| targets | pcbench-abus-cfa1000-display-grabber_acs-display-grabber | 0 → 0 | 0 → 0 | 3 → 2 | False |
| targets | pcbench-amalthea_amalthea_rev0 | 23 → 20 | 0 → 0 | 200 → 199 | True |
| targets | pcbench-atmegax8-protoboard_atmegax8-protoboard | 0 → 0 | 0 → 0 | 41 → 37 | False |
| targets | pcbench-audprog_audprog_v2 | 0 → 0 | 0 → 0 | 33 → 35 | False |
| targets | pcbench-avr-fuser-32_adapter | 0 → 0 | 0 → 3 | 204 → 207 | False |
| targets | pcbench-avr_ledprojector_avr_ledprojection-0402 | 1 → 0 | 4 → 2 | 0 → 0 | False |
| targets | pcbench-azalea_azalea | 41 → 36 | 55 → 51 | 159 → 159 | True |
| targets | pcbench-badge2016_Badge_init | 0 → 0 | 0 → 0 | 239 → 211 | False |
| targets | pcbench-balena-rover-wide-hat_resin-rover | 0 → 0 | 4 → 2 | 200 → 203 | False |
| targets | pcbench-bikedar_bikedar | 0 → 0 | 4 → 2 | 0 → 0 | False |
| targets | pcbench-blackmagic-isolated_mmp | 1 → 0 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-bms-8s50-ic_bms-8s50-ic | 54 → 57 | 0 → 0 | 88 → 88 | True |
| targets | pcbench-boatcontrol_NonLatchingNO30A | 27 → 16 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-bobc_MS-F100 | 0 → 0 | 0 → 0 | 189 → 154 | False |
| targets | pcbench-breakout-boards_esp8266-jtag | 0 → 0 | 0 → 0 | 30 → 22 | False |
| targets | pcbench-cnlohr_wiflier | 17 → 14 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-decelerator4030_decelerator4030 | 474 → 499 | 10 → 15 | 0 → 0 | True |
| targets | pcbench-disco-dongle_DiscoDongle | 0 → 0 | 1 → 4 | 0 → 0 | False |
| targets | pcbench-dorkyboard_keyboard | 3 → 1 | 20 → 0 | 204 → 201 | False |
| targets | pcbench-epapercard_epapercard | 0 → 0 | 0 → 0 | 137 → 129 | False |
| targets | pcbench-esp32-4-channel-relays_esp32-4-channel-relays | 7 → 9 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-esp32-ethernet_esp32-ethernet | 0 → 0 | 3 → 1 | 0 → 0 | False |
| targets | pcbench-espalarm_alarm | 0 → 0 | 73 → 76 | 0 → 0 | False |
| targets | pcbench-esper_EsperDNS | 9 → 10 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-fifogfx_c64cart | 2 → 2 | 0 → 0 | 81 → 80 | False |
| targets | pcbench-freeDSP-CLASSIC-SMD-BALANCED_FreeDSP_BAL | 0 → 1 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-ftdi-jtag-programmer_JTAGProgrammer | 12 → 9 | 2 → 2 | 0 → 0 | False |
| targets | pcbench-gb-hardware_GB-CART256K-A | 14 → 10 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-gb-hardware_GB-LIVE32 | 3 → 6 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-gb-hardware_GB-MBCTEST | 2 → 0 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-hbr-mk2_hbr-mk2-bpfs | 7 → 11 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-headstage-adapter_headstage adapter | 0 → 0 | 0 → 0 | 204 → 219 | False |
| targets | pcbench-juno-chorus-clone_juno-chorus-clone | 0 → 0 | 2 → 4 | 0 → 0 | False |
| targets | pcbench-karabas-nano_karabas-nano-revA | 29 → 54 | 0 → 0 | 198 → 198 | True |
| targets | pcbench-karabas-nano_karabas-nano-revB | 64 → 90 | 0 → 0 | 199 → 199 | True |
| targets | pcbench-karabas-nano_karabas-nano-revC | 81 → 97 | 1 → 1 | 200 → 199 | True |
| targets | pcbench-karabas-nano_karabas-nano-revG | 85 → 92 | 0 → 0 | 199 → 199 | True |
| targets | pcbench-kassenautomat.mdb-interface_mdb-interface | 0 → 0 | 0 → 0 | 191 → 183 | False |
| targets | pcbench-keyboards_Djinn | 386 → 384 | 0 → 0 | 0 → 0 | True |
| targets | pcbench-kinetoscope_microcontroller | 52 → 57 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-kinetoscope_sram-bank | 112 → 111 | 0 → 0 | 0 → 0 | True |
| targets | pcbench-kitspace_40-channel-hv-switching-board | 0 → 1 | 0 → 0 | 202 → 203 | True |
| targets | pcbench-kitspace_CH330 | 1 → 3 | 0 → 0 | 5 → 5 | False |
| targets | pcbench-kitspace_DIY_detector | 0 → 0 | 1 → 5 | 8 → 8 | False |
| targets | pcbench-kitspace_OSO-BOOK-C1 | 7 → 7 | 32 → 33 | 0 → 0 | False |
| targets | pcbench-kitspace_T32_ref | 0 → 0 | 0 → 0 | 181 → 171 | True |
| targets | pcbench-kitspace_dropbot-front-panel | 0 → 9 | 0 → 0 | 208 → 201 | True |
| targets | pcbench-kitspace_dropbot_control_board | 0 → 0 | 0 → 0 | 202 → 201 | False |
| targets | pcbench-kitspace_esp8266 | 0 → 0 | 0 → 0 | 78 → 54 | False |
| targets | pcbench-kitspace_minisumo_v3 | 1 → 1 | 1 → 2 | 22 → 22 | False |
| targets | pcbench-kitspace_sympetrum-v2%20NFF1.1 | 13 → 13 | 0 → 0 | 171 → 173 | False |
| targets | pcbench-led-wordclock_wordclock | 0 → 0 | 5 → 3 | 76 → 76 | False |
| targets | pcbench-m2-electronics_m2fc | 1 → 0 | 11 → 15 | 0 → 0 | True |
| targets | pcbench-m2-electronics_m2r | 0 → 1 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-mavbridge_mavbridge | 0 → 0 | 0 → 0 | 11 → 8 | False |
| targets | pcbench-mdbwerk_mdbwerk | 0 → 0 | 2 → 5 | 0 → 0 | False |
| targets | pcbench-memsarray_mems_array | 0 → 0 | 0 → 0 | 201 → 217 | False |
| targets | pcbench-mightyduino_mightyduino | 1 → 0 | 0 → 0 | 67 → 61 | False |
| targets | pcbench-mobile-sensor-pcb_mobile-sensor-pcb | 0 → 0 | 2 → 0 | 48 → 48 | False |
| targets | pcbench-mppt-2420-hc_mppt-2420-hc | 1 → 1 | 4 → 0 | 4 → 0 | False |
| targets | pcbench-mppt-2420-hpx_mppt-2420-hpx | 6 → 7 | 2 → 3 | 2 → 3 | False |
| targets | pcbench-nodemcu-basecamp_NodeMCU Basecamp | 0 → 0 | 0 → 1 | 0 → 0 | False |
| targets | pcbench-nonSNES_SNSP-CPU-1CHIP | 159 → 203 | 1 → 0 | 28 → 1 | True |
| targets | pcbench-ottawa-badges-2016_ottawa-badge-tagger-2016 | 0 → 0 | 0 → 0 | 3 → 9 | False |
| targets | pcbench-pcb-usb-ft245r-parallel-adapter_pcb-usb-ft245r-parallel-adapter | 0 → 0 | 0 → 0 | 40 → 45 | False |
| targets | pcbench-pico-pi-rel_pico-pi | 0 → 0 | 0 → 0 | 203 → 199 | False |
| targets | pcbench-preamp-two_input-selector | 0 → 1 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-project-hydra-meshtastic-pcb_meshtastic-diy | 24 → 28 | 5 → 7 | 2 → 2 | False |
| targets | pcbench-pusheenz40_sadcatz40 | 43 → 45 | 0 → 0 | 0 → 0 | True |
| targets | pcbench-real-time-chess_kfchess | 78 → 81 | 70 → 86 | 0 → 0 | True |
| targets | pcbench-rjw57_cpu-board | 0 → 0 | 0 → 2 | 0 → 0 | True |
| targets | pcbench-roomba-ESP12E_roomba-esp | 0 → 0 | 3 → 1 | 6 → 6 | False |
| targets | pcbench-rp2040-dmxsun_baseboard_2slots | 0 → 0 | 8 → 9 | 0 → 0 | False |
| targets | pcbench-rp2040-dmxsun_baseboard_4slots | 0 → 0 | 3 → 8 | 4 → 4 | False |
| targets | pcbench-rxadc_14_rxadc_14 | 1 → 1 | 0 → 0 | 149 → 150 | False |
| targets | pcbench-srambo_1_srambo_1 | 1 → 0 | 0 → 0 | 136 → 177 | False |
| targets | pcbench-starfish_starfish | 1 → 0 | 0 → 0 | 0 → 0 | False |
| targets | pcbench-stm32_ccd_camera_ccd | 0 → 0 | 0 → 1 | 71 → 71 | False |
| targets | pcbench-timecircuits-hardware_BTTF-TimeCircuits | 0 → 0 | 0 → 0 | 205 → 212 | False |
| targets | pcbench-wavegen_rev3 | 12 → 5 | 0 → 0 | 201 → 202 | True |
| targets | pcbench-wavegen_waveform-generator-rev1 | 1 → 1 | 0 → 0 | 209 → 202 | False |
| targets | pcbench-wavegen_wavegen | 1 → 1 | 0 → 0 | 202 → 200 | False |
| targets | pcbench-zx-sizif-512-ext_sizif512ext | 58 → 52 | 0 → 0 | 183 → 183 | True |
| targets | pcbench-zx-sizif-xxs_sizif-xxs | 27 → 130 | 0 → 0 | 165 → 165 | True |
| targets | kicad-issue184-motorizedopener--motorizedopener | 61 → 64 | 54 → 54 | 0 → 0 | False |
| targets | kicad-issue269-nowiresonpowerlayers--proba | 1 → 0 | 0 → 0 | 0 → 0 | True |
