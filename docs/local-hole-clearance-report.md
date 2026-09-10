# Local clearance around mechanical holes

KiCad exports some non-plated mounting holes as circular DSN package keepouts rather than pins. The pin metadata loader therefore never applies their local copper-clearance overrides. Balena's 2.75mm holes require 1.725mm clearance; its DSN keepout diameter is 3.25mm, already including 0.25mm radial padding. Esp32's 3.3mm holes require 1.65mm clearance. Both original boards are fully connected and have zero routing DRC.

This prototype adds optional `hole_diameter_um` metadata for round non-plated holes with pad size no larger than the drill. A component/position-matched circular keepout containing that drill receives an additional clearance floor:

`max(drill_radius + local_copper_clearance − keepout_radius, 0)`

The result rounds upward in board units and preserves higher existing rules. For the balena example it requests 1.475mm beyond the existing keepout edge, producing the required 1.725mm total clearance from the hole edge. The separate obstacle entry point shares the existing clearance-class floor logic while retaining the original pin-only guard. Clearance-class changes clear derived data and reinsert compensated obstacle entries through the existing board API.

Missing diameters leave existing metadata behavior intact. Nonpositive/nonfinite/out-of-range diameters fail validation before matching; wrong components, positions and diameters larger than the keepout are not applied. Slots and noncircular keepouts are outside this prototype. This does not change the board-wide hole-clearance default.

## Input verification

The generated metadata retains every field from the already measured pad/reference sidecars and adds hole diameters to 183 records on 22 of 751 boards. Validation checks all 751 files, rejects other changes and records file hashes. The full run uses current main 7d33cef, a same-binary control with old metadata, and the candidate with annotated metadata. Builds and runs are serialized on the server; ten passes, 300-second cap, one routing thread, 192 jobs. All 2,253 KiCad referee results succeeded. Details are backed up on the laptop and workbench.

## Initial pilot before the image fix merged

`quality-epyc-local-hole-pilot-01`, base 028f0a5: all 12 KiCad referees successful and all routes completed. Main and the same-binary control match quality on all four boards. These are selected diagnostic boards, not a corpus-wide result.

| Board | Unrouted control→candidate | Copper DRC control→candidate | CPU seconds control→candidate |
|---|---:|---:|---:|
| esp32-ethernet | 0→1 | 1→0 | 74.63→85.69 |
| balena-rover | 0→0 | 2→0 | 93.81→82.65 |
| 96boards Sensors | 0→0 | 0→0 | 153.87→151.07 |
| OpenHardwareExG Shield | 1→1 | 30→30 | 106.65→108.52 |

The pilot trade is +1 unrouted/−3 copper, with no copper gainers and one connectivity regression. It verifies that the previously unmatched rules can affect routing and clear the targeted violations; it does not establish a connectivity or speed improvement. Original-board and pilot artifacts are retained under `routing-quality-artifacts/local-hole-clearance` and copied to workbench.

## Validation status

A failing-first DSN test reproduced 0.25mm stored clearance where 1.475mm additional clearance was required. Six focused loader tests cover correct padding, preservation of higher rules, metadata matching guards, invalid diameters and existing pin behavior. The first full current-main workspace run passed 2,591 tests, 77 ignored, zero failed, including doctests. A later assertion additionally checks that the expanded obstacle is returned by the actual clearance query; the fresh workspace run also passed 2,591 tests, 77 ignored, zero failed, including doctests. Production source is unchanged from the frozen full-run candidate. The generated benchmark regression gate reports seven quality losses; these remain visible in the accompanying report.

## Full current-main run

`quality-epyc-local-hole-full-01`, main 7d33cef, 751 KiCad boards per candidate:

| Comparison / group | Unrouted delta (better/worse boards) | Copper delta (gainers) | Mask delta (gainers) | CPU ratio | Completed baseline→candidate |
|---|---:|---:|---:|---:|---:|
| Same-binary control→holes, 740 PCBench | −433 (16/3) | −8 (2) | +639 (8) | 0.9481 | 712→717 |
| Same-binary control→holes, 11 local | 0 (0/0) | 0 (0) | 0 (0) | 0.9975 | 10→10 |
| Main→holes, 740 PCBench | −464 (15/3) | −8 (2) | +34 (10) | 0.9478 | 709→717 |
| Main→holes, 11 local | 0 (0/0) | 0 (0) | 0 (0) | 1.0038 | 10→10 |

The large raw connectivity changes are deadline-sensitive and are not evidence of hundreds of recovered connections. On 722 completed control/candidate pairs the change is **0 unrouted / −9 copper**: ESP32 recovers one connection and removes two hole violations; balena removes three; bikedar removes four; EncoderBoard loses one connection. Three boards improve copper and none gain copper violations among completed pairs. This is a local-rule correctness improvement with a real connectivity trade on EncoderBoard, not a demonstrated general connectivity improvement.

Of these completed pairs, 712 have byte-identical SES outputs, accounting for all +612 mask-count changes; the ten changed outputs have zero net mask change. Main and control produce identical SES on all 719 completed pairs, with identical connectivity/copper and −628 mask-count difference. This directly demonstrates referee variability on unchanged geometry. All raw counts remain reported; they must not be interpreted as a mask regression caused by changed routes. Main/candidate completed pairs likewise show 0 unrouted / −9 copper, with −30 mask on 710 identical outputs and zero on nine changed outputs.

Median/max PCBench RSS is 12.0/373.9 MiB for control and 12.0/372.4 MiB for holes. Local median/max is 13.5/78.2→13.5/72.8 MiB. Single-run CPU ratios are descriptive, not a speed claim. Fully connected PCBench boards fall 590→589; local stays eight.

## EncoderBoard investigation

This is an ordinary regression, not an outlier exemption. The original has zero unrouted/copper DRC and 39 mask reports. Its four MP1–MP4 mounting holes are 3.2mm with 0.6mm local copper clearance. The remaining candidate connection is `/LED14`, IC1 pad 3 to R18 pad 2. The original routes that net with two vias and 28.86mm of track; control uses four vias and 32.10mm; candidate leaves it entirely unrouted. Both existing routes stay more than 10mm from the four annotated holes (10.271mm original, 10.304mm control, edge-to-edge). The new hole boundary therefore does not directly block those routes; changed routing of other nets is an indirect interaction still to investigate. Whole-board candidate has fewer vias (39→26) and less wire (1046.91→966.35mm), but those totals include the missing net and do not establish a quality improvement.

## EncoderBoard pass-budget diagnostic

`quality-epyc-local-hole-encoder-20pass-01` reroutes only EncoderBoard with the same frozen control/holes executables and metadata, 20 passes, a 600-second diagnostic cap, and two single-thread jobs. Both complete, with successful KiCad referees, **0 unrouted and 0 copper violations**. Control/candidate CPU is 108.76/100.71 seconds, vias34/39 and wire1046.24/1058.25mm. This shows the missing connection can be recovered with further routing; it does not replace the 10-pass acceptance comparison or justify a general runtime claim or global budget increase. The 10-pass +1U regression remains reported.
