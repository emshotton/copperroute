# KiCad solder-mask DRC accuracy

The candidate adds checks for bridges involving unnetted copper and between pad openings, carries the board minimum mask web and footprint permission through native import, distinguishes copper-free mounting holes from square pads retaining copper, and preserves logical pad identity when the browser places physical pads independently.

Independent KiCad 10.0.6 fixtures cover the new checks and their exceptions. The workspace suite passes 2,617 tests, with 77 ignored and no failures; 52 browser tests pass. Existing JVM recordings were not replaced.

## Native-board comparison

Both Rust versions check identical already-routed boards from `structural-full-01`, imported through the browser adapter with the original project rules. No rerouting is involved. Results and source snapshots are backed up to workbench under `copperroute-epyc-results/web-structural-routing/server/`.

| Measure | Result |
|---|---:|
| KiCad boards attempted | 751 |
| Reports produced by both Rust versions | 549 |
| Unsupported imports | 202 |
| Paired boards below KiCad's mask finding cap | 529 |
| Boards with closer mask counts to KiCad | 119 |
| Boards with farther mask counts from KiCad | 0 |
| KiCad pad-pair mask findings on those 529 boards | 4,509 |
| Spatially matching pad-pair findings on main | 0 |
| Spatially matching pad-pair findings on candidate | 4,413 |

These are separate measurements: closer counts do not establish identical findings. The pair diagnostic compares unordered pad positions within 0.001 mm and retains multiplicities, but does not establish UUID/layer-perfect agreement. There are 36 unmatched candidate pairs and 96 unmatched KiCad pairs in the unmodified reports. Twenty paired boards hit KiCad's 199-mask-finding cap and are excluded only from this uncapped accuracy analysis, not the routing benchmark.

The six new false warnings initially observed on Feather ICE40 were caused by losing the identity of the four pieces of U5 pad 0. Preserving source footprint and pad number removes all six; native tests cover numbered versus unnumbered pads and different footprints.

Remaining unmatched candidate pairs are concentrated on three boards:

- `pcbench-zx-sizif-xxs_sizif-xxs`: ten findings around rotated resistor-array pads whose mask openings nominally just touch. Router-grid precision is under investigation; these remain a disclosed accuracy limitation.
- `pcbench-kitspace_sensor`: 22 position mismatches, with explicit 0.15 mm copper offsets. KiCad reports the pad anchor while Rust reports its copper center.
- `pcbench-rs485-moist-sensor_adapter-por`: four mismatches caused by duplicated source UUIDs. A UUID-only diagnostic preserves all 22 mask findings and makes all eight pad-pair findings match. See [the outlier register](outlier-boards.md).

Full KiCad parity is not established. Unsupported imports, via mask apertures, mask artwork, net ties, custom rules, zone-fill/thermal connectivity and precise geometry remain gaps. Zone fills are excluded from the native model in this survey. KiCad remains the acceptance referee.

## Routing compatibility

The final source was evaluated as `drc-parity-mask-02` against pristine main `736d02d`, the latest fetched main. Both sides use the same 751 KiCad boards, ten passes, a 300-second deadline and 112 single-thread workers on the same server. CPU time is the timing measure; no speed claim is made from one repetition.

This DSN benchmark checks routing compatibility, not mask recall: DSN does not carry native mask metadata. All 751 boards were scored on both sides with zero referee failures. The unchanged latest main was fetched before validation.

| Metric | Main | Candidate |
|---|---:|---:|
| Unrouted connections | 5,112 | 4,838 |
| Ordinary copper errors | 871 | 857 |
| Solder-mask errors | 13,731 | 13,747 |
| Fully connected boards | 600 | 601 |
| Router deadlines | 54 | 43 |
| CPU seconds | 46,992.00 | 45,106.75 |
| Maximum per-board RSS, MiB | 291.5 | 293.2 |

| KiCad scope | Boards | U improved/regressed | ΔU | Copper-gaining boards / Δcopper | Mask-gaining boards / Δmask | CPU ratio |
|---|---:|---:|---:|---:|---:|---:|
| All boards | 751 | 17/4 | -274 | 1 / -14 | 2 / +16 | 0.9599 |
| Both completed | 696 | 0/0 | +0 | 0 / +0 | 0 / +0 | 0.9568 |
| PCBench, all | 740 | 17/4 | -274 | 1 / -14 | 2 / +16 | 0.9590 |
| PCBench, both completed | 687 | 0/0 | +0 | 0 / +0 | 0 / +0 | 0.9562 |
| Local fixtures, all | 11 | 0/0 | +0 | 0 / +0 | 0 / +0 | 0.9991 |
| Local fixtures, both completed | 9 | 0/0 | +0 | 0 / +0 | 0 / +0 | 0.9977 |

All changes to unrouted and violation counts involve a router deadline on at least one side; the 696 pairs finishing on both sides have identical quality. Router deadlines come from result.json, not just the benchmark's external timeout flag. No Java referee was used.

The benchmark gate fails on six boards: four deadline-limited unrouted losses (Karabas revB +6, Librecalc autosave +2, RC6502 VDU +1, ReSDMAC +1) and two small wirelength/via-score losses with unchanged unrouted and copper errors (96boards Sensors and 40-channel HV switch). All six hit the deadline in both runs. A one-pass, 1200-second control with optimization disabled on both versions completed on all six without deadlines. Each pair produced byte-identical SES output and identical KiCad findings. This isolates the first routing pass from the optimizer and time cutoff; it does not prove every later pass is identical. The original full-run gate failure remains disclosed. [Control results](routing-quality-artifacts/drc-parity/drc-mask-one-pass-no-opt-control.json).

The aggregate CPU ratio is 0.9599 while the benchmark median per-board ratio is 1.00. One repetition does not establish a speed improvement. Full generated output, including the failing gate and timing-noise warnings: [benchmark PR summary](routing-quality-artifacts/drc-parity/drc-parity-mask-02-pr.md).
