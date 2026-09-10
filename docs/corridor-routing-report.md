# Shared corridor guidance with final path validation

Corridor guidance encourages related signals to share a routing band and fan out near their terminals. It is now enabled by default, including the final path check after all shoves. A later shove can move another net into an earlier checked segment; that check rejects the path before inserting copper. Existing checks remain in place. Set `COPPERROUTE_CORRIDOR_GUIDANCE=0` to disable guidance; unset or `1` enables it. Protected-prefix routing is a separate, unlanded experiment.

The user requested default activation after reviewing the measured trade. A new four-way corpus run compares previous main, previous opt-in guidance, the new default, and the explicit opt-out. See [the default activation report](corridor-default-report.md).

## Original opt-in validation

The final comparison against main **54d4e80** gives **36 fewer unrouted connections and zero net copper DRC increase on 719 completed pairs**. Connections improve on 29 boards and regress on 13; copper improves on seven and increases on seven. These ordinary regressions remain counted. These were the results supporting the original opt-in landing; default activation is documented separately above.

`quality-epyc-shove-coupled-full-01`: main, same-binary disabled control, and guidance with its automatic guard; 751 boards per candidate, all 2,253 KiCad referees successful. Ten passes, 300-second cap, one routing thread, 192 workers. All candidates use the same project minimums and merged local-hole/pad metadata. Failed referees were rescored on unchanged SES outputs. Results are preserved on laptop and workbench. No Java DRC is used.

| Comparison / population | Raw delta U (better/worse) | Raw delta copper (gainers) | Raw delta mask (gainers) | CPU ratio | Completed pairs | Paired delta U / copper |
|---|---:|---:|---:|---:|---:|---:|
| Main → guidance / pcbench | -190 (44/16) | -1 (8) | -107 (13) | 0.9431 | 709 | -39 / -1 |
| Main → guidance / local | +4 (0/2) | +1 (1) | +0 (0) | 1.0108 | 10 | +3 / +1 |
| Control → guidance / pcbench | -155 (44/17) | -3 (8) | -100 (10) | 0.9410 | 710 | -39 / -1 |
| Control → guidance / local | +4 (0/2) | +1 (1) | +0 (0) | 1.0045 | 10 | +3 / +1 |

PCBench contributes −39 U/−1 copper on 709 completed main pairs; local fixtures contribute +3 U/+1 copper on ten pairs. The same-binary comparison has 720 completed pairs and the same −36 U/0 copper total. Raw unrouted totals include deadline effects and are not claimed as recovered connections. CPU ratios are observations from one repetition, not demonstrated speedups.

| Population, main → guidance | Finished | Unfinished | Fully connected | Median peak RSS MiB | Max peak RSS MiB |
|---|---:|---:|---:|---:|---:|
| pcbench | 709→716 | 31→24 | 589→599 | 12→12.1 | 370.9→372.4 |
| local | 10→10 | 1→1 | 8→7 | 13.5→12.1 | 67.3→124.5 |

All 719 completed main/control SES outputs are byte-identical (0 U/0 copper/−27 mask). Main/guidance has 492 identical outputs (0 U/0 copper/−14 mask) and 227 changed outputs (−36 U/0 copper/−107 mask). All 726 completed outputs shared with the earlier five-way combined experiment are also byte-identical. These checks confirm disabled behavior and automatic-guard behavior. Mask changes on identical outputs cannot be attributed to routing; raw counts remain disclosed.

## Mechanism and regression review

OpenHardwareExG Shield exposed the defect: a late shove moves AGND back across earlier signal segments. Corridor-only routing produces 12 shorts and eight clearance violations in addition to 34 track-width errors. The guarded result removes all 20 new short/clearance violations, leaving 3 U/34 width errors versus main's 1 U/30 width errors. All width errors involve 0.2488 mm tracks against a 0.2540 mm project minimum. Its original is fully connected with zero routing DRC; it is not treated as an outlier.

The guard adds six unrouted connections on boatcontrol_NonLatchingNO30A and three on uSKY versus corridor alone. Both originals are fully connected with zero copper DRC. Captured replays reject other-net obstructions; uSKY's proposed route shares four exact centerline segments with another net. Logging and capture runs reproduce the scored SES byte for byte. Final main outputs having zero copper errors does not establish that every intermediate insertion was safe. These losses are retained, not waived.

The seven completed copper gainers are Shield (+4 width), VC4000 (+2 shorts against copper text), rp2040-dmxsun baseboard_2slots (+3 starved thermals), Patternflow (+1 starved thermal), ESP07 (+1 clearance), pmw3360_jst (+1 copper-edge clearance), and motorizedopener (+1 hole clearance). The last board also has +3 U. Azalea remains +4 U with zero copper DRC on both sides.

VC4000 improves 4→2 U while copper rises 11→13. KiCad identifies its back-copper text in the short reports. The DSN omits that text geometry: no text string, wire or keepout; its 13 polygons are footprint outlines. This confirmed input-fidelity outlier is recorded in [the outlier register](outlier-boards.md), but its losses remain in every headline total. Other regressions are not assumed to be outliers.

## Validation and limits

Failing-first tests reproduce the unsafe insertion and prove that enabling guidance alone enables its required guard. A clear multi-segment path still reaches its endpoint. Fresh `cargo test --workspace` on the actual PR branch passed **2,599 top-level tests plus three isolated child checks, 77 ignored, zero failures**. All **602 source/data/test/fixture hashes** match the frozen benchmark candidate. No JVM recordings changed. Temporary instrumentation and the diagnostic production dependency on copper-dsn are excluded.

The generated benchmark gate still fails **112 routing-quality losses**. This is an explicit trade, not a clean gate. The native web-import path has not received equivalent corridor validation. The known connection and copper regressions remain counted in the default-activation assessment.

Artifacts: [full generated summary](routing-quality-artifacts/shove-revalidation/coupled-pr-summary.md), [comparisons](routing-quality-artifacts/shove-revalidation/coupled-comparisons.json), [session identities](routing-quality-artifacts/shove-revalidation/coupled-identities.json), [source hashes](routing-quality-artifacts/shove-revalidation/quality-shove-coupled-source-hashes.json), [fresh PR suite](routing-quality-artifacts/shove-revalidation/pr27-workspace-summary.txt), and [mechanism investigation](shove-revalidation-report.md). Earlier and negative experiments remain in [the experiment table](routing-experiment-comparison.md) and [running log](routing-quality-log.md).
