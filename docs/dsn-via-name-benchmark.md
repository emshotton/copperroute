# Fractional via-name import correction

DSN library import normalizes fractional numbers in padstack names, but net-class `use_via` previously searched only the original name. A declaration such as `Via[0-1]_685.8:330.2_um` therefore produced an empty class via rule after the library stored `Via[0-1]_685:330_um`. The main router lost the declared via option even though fanout could still use the global catalog.

The change accepts either the exact name or the existing library normalization. It preserves the clearance-class filter and SMD-attachment guard; it does not change via geometry or introduce new via sizes. A regression test first failed with zero available vias instead of one, then passed for ordinary and KiCad-default class names. Three JVM comparison tests explicitly assert the corrected nonempty rule while retaining every original recording and all other comparisons.

## Full KiCad benchmark

Run `quality-epyc-via-01` compared latest main `e9d10c21da58653fb5332df44021f25180a3d4da` with the frozen production patch, on the EPYC 9654 server. Settings: 751 KiCad boards (740 PCBench, 11 local fixtures), 192 jobs using hyperthreading, one router thread, ten passes, 300-second deadline, one repetition, KiCad 10.0.4. All 1,502 cells have successful referee results. Failed initial referee invocations were retried on unchanged SES files; warning-prefixed structured output was recovered only when the final output line was the unique JSON object. No routing was repeated for those repairs.

| Group | Unrouted main → change | Boards improved / regressed (unrouted) | Copper DRC main → change | Boards gaining copper DRC | Reported mask main → change | Boards gaining mask | Total CPU ratio |
|---|---:|---:|---:|---:|---:|---:|---:|
| PCBench / KiCad, 740 | 5,260 → 4,514 | 53 / 1 | 940 → 917 | 1 | 15,183 → 15,221 | 26 | 0.960 |
| Local / KiCad, 11 | 240 → 239 | 1 / 0 | 59 → 59 | 0 | 0 → 0 | 0 | 1.021 |

| Whole corpus | Main | Change |
|---|---:|---:|
| Unrouted connections | 5,500 | 4,753 |
| Copper DRC violations | 999 | 976 |
| Reported solder-mask violations | 15,183 | 15,221 |
| Fully connected boards | 527 | 549 |
| Completed routing jobs / deadline-limited jobs | 721 / 30 | 729 / 22 |
| Total router CPU, seconds | 37,858.52 | 36,390.59 |
| Median per-process peak RSS, MiB | 13.6 | 13.6 |
| Maximum per-process peak RSS, MiB | 447.5 | 451.9 |

Completed jobs can still contain unrouted connections. Copper DRC is the benchmark's routing-violation subset; solder mask is reported separately and is not silently treated as acceptable. The aggregate CPU ratio is descriptive, not a speed claim: there is only one repetition and deadline outcomes vary with contention. The main binary was built on workbench; the candidate used the same Rust toolchain on the EPYC host. A fresh same-source main build produced byte-identical SES on all 721 pairs that completed in both build comparisons, supporting functional equivalence but not timing equivalence.

## Regressions and limits

- The benchmark regression gate **fails: 28 routing-quality losses**. Its composite score also counts small route-cost changes; this is not a claim that 28 boards gain unrouted connections. The unmodified generated summary is preserved separately, and the PR retains the failed gate.
- Apple M0110 is the only board with increased unrouted connections: **0 → 1**, both jobs completed. The missing connection is on Col9. Restored via availability changes the routing sequence: vias rise from zero to two, length falls from 4,356.0 to 4,320.1 mm, and CPU rises from 9.97 to 35.89 seconds. The original board is fully connected with 14 vias. A separate failed-insertion recovery prototype recovers this board locally, but has other corpus regressions and is not included.
- nonSNES SNSP-CPU-1CHIP is the only copper-DRC gainer, **+2**, while unrouted falls from 185 to 132. Both jobs hit the deadline.
- Mask counts increase by 38 overall. Of 751 pairs, 638 have identical SES, with zero unrouted delta but **+26 reported mask counts** from referee/report-limit variability. The 113 different-SES pairs account for the remaining **+12 mask counts**. This does not erase actual mask regressions: uncapped examples include kitspace ESP8266 **53 → 71**, bobc MS-F100 **140 → 165** (unrouted 12 → 0), teensy-touch **16 → 26**, and rxadc14 **156 → 163**. Mask-aware routing remains a separate experiment.
- Among the 721 pairs where both routers completed, unrouted improves by **452**; the remaining deadline-affected pairs contribute **295** of the aggregate 747-connection improvement. Improvements therefore do not rely solely on deadline outcomes.

## Validation and provenance

`RUSTC_WRAPPER= CARGO_TARGET_DIR=../copperroute-via-name/target cargo test --workspace` completed successfully before commit: **2,567 passed, zero failed, 77 ignored**. The production file SHA-256 is `f69f6ccc2ed63ffde22d10e62757ba58e617dec2bdbad74a0295b080ceeafa4d`, identical to the measured frozen candidate. Formatting and explicit JVM-test expectations do not change the benchmarked production code.

Full results, SES files, referee output and initial referee failures are backed up to workbench at `/home/em/copperroute-epyc-results/results/quality-epyc-via-01`; reports and exports are also copied to the laptop's `copperroute-epyc-results` directory. No Java referee results are included.

## Benchmark tool summary

The following is the tool output with only 751 repetitive one-repetition warnings condensed. The full output is in `routing-quality-artifacts/via-name/bench-pr-summary.md`.

## Benchmark: via-name vs main

**worse** — the regression gate **fails**:

- via-name: 28 routing-quality losses

| metric | result |
|---|---|
| Boards compared | 751 of 751 shared |
| Wins / losses / ties | 198 / 521 / 32 |
| Quality losses | 28 |
| Performance losses | 0 (advisory) |
| Clean-pass rate | 0.65 → 0.68 |
| Median Δscore | 0.0 |
| Median time ratio | 1.02 |

<details><summary>719 boards changed</summary>

| board | verdict | Δunrouted | Δviol | Δscore | Δcpu s |
|---|---|---|---|---|---|
| pcbench-Apple-M0110-BT_Apple M0110 | loss (clean_pass_rate) | +1 | 0 | -8.6 | +25.9 |
| pcbench-HaveSome_PCB_HaveSomePCB | loss (score) | 0 | 0 | -0.0 | +0.3 |
| pcbench-scimpy_amp | loss (score) | 0 | 0 | -0.0 | -5.4 |
| pcbench-kitspace_esp8266 | loss (score) | 0 | 0 | -0.0 | +0.5 |
| pcbench-medusa_medusa_rs422_rx | loss (score) | 0 | 0 | -0.0 | -4.4 |
| pcbench-LED-Square_PT4115_LED-Square_PT4115 | loss (score) | 0 | 0 | -0.0 | -2.3 |
| pcbench-MySRaspiGW_MySRaspiGW_Pimoroni | loss (score) | 0 | 0 | -0.0 | +3.4 |
| pcbench-scimpy_crossover | loss (score) | 0 | 0 | -0.0 | +4.2 |
| pcbench-DasBlinkinput_Das Blinkinput | loss (score) | 0 | 0 | -0.0 | -41.8 |
| pcbench-rxadc_14_rxadc_14 | loss (score) | 0 | 0 | -0.0 | +34.8 |
| pcbench-Hypfer-RGB-W-LED-Controller_led_strip_controller | loss (score) | 0 | 0 | -0.0 | -4.1 |
| pcbench-esp12-appliance_mod_esp12-appliance-mod | loss (score) | 0 | 0 | -0.0 | -14.4 |
| pcbench-FT231X_breakout_FTDI_FT231XS-U_Breakout | loss (score) | 0 | 0 | -0.0 | +11.5 |
| pcbench-Practicas-Curso-Kicad_Salguero_Federico2 | loss (score) | 0 | 0 | -0.0 | -13.4 |
| pcbench-NRC2016_usb_sio | loss (score) | 0 | 0 | -0.0 | -86.0 |
| pcbench-scimpy_volumebuffer | loss (score) | 0 | 0 | -0.0 | -1.8 |
| pcbench-DiscoDanceFloorV1_DiscoDongle | loss (score) | 0 | 0 | -0.0 | -30.0 |
| pcbench-ezusb-logicanalyzer_cypress_logic_analyzer | loss (score) | 0 | 0 | -0.0 | +4.6 |
| pcbench-polypoint_pinpoint_timebase | loss (score) | 0 | 0 | -0.0 | -4.7 |
| pcbench-ESP8266-MQTT-battery-monitor-hw_battery-monitor | loss (score) | 0 | 0 | -0.0 | -29.0 |
| … | and 699 more | | | | |

</details>

Baseline first: main `e9d10c21da58653fb5332df44021f25180a3d4da` · via-name `e9d10c2+normalized-use-via-name`

threads=1 jobs=192 max_passes=10 timeout=300s seeds=1, deciding on cpu s. Runs quality-epyc-via-01.

**Caveats**

- seeds=1 (<3), so the noise floor is unmeasured and timing differences here are not evidence.

- Condensed 751 per-board warnings: each baseline has one judged repetition; timing noise is unmeasured.

<sub>Generated by `uv run bench pr-summary --compare quality-epyc-via-vs-main` from `reports/quality-epyc-via-vs-main.json`.</sub>
