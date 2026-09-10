# Shared corridor guidance

This opt-in routing experiment encourages related signals to travel through a shared band and fan out at their endpoints. The latest comparison retains a connectivity benefit but adds copper violations, concentrated on OpenHardwareExG Shield. Keep the PR as a draft; this is not ready for default activation or merge as a quality improvement.

## Current main 7d33cef comparison

`quality-epyc-corridor-current-main-01`: 751 KiCad boards each for main, same-binary guidance-disabled control, and guidance-enabled candidate. All 2,253 referees succeeded after retrying failed checks on unchanged SES. Ten passes, 300-second cap, one routing thread, 192 jobs; identical project-minimum and pad/reference metadata for every candidate. Results are saved on laptop and workbench.

| Comparison / population | Δ unrouted (better/worse) | Δ copper (gainers) | Δ mask (gainers) | CPU ratio |
|---|---:|---:|---:|---:|
| Control→guidance, 740 PCBench | −340 (46/15) | +18 (10) | −66 (13) | 0.9385 |
| Control→guidance, 11 local | +4 (0/2) | +1 (1) | 0 (0) | 1.0119 |
| Main→guidance, 740 PCBench | −375 (46/14) | +20 (10) | −86 (12) | 0.9416 |
| Main→guidance, 11 local | +4 (0/2) | +1 (1) | 0 (0) | 1.0184 |

On 720 completed control/candidate pairs: **−45 unrouted / +22 copper**. PCBench contributes −48 U (30 improved, 10 regressed) and +21 copper (eight gainers); local contributes +3 U/+1 copper on motorizedopener. Main/candidate has the same completed U/copper delta on 719 pairs. The large raw unrouted deltas contain deadline effects and are not claimed as recovered connections. Single-run CPU ratios do not establish speed.

Of completed control/candidate pairs, 498 SES outputs are byte-identical (0U/0Cu/−11mask), and 222 differ (−45U/+22Cu/−66mask). All 719 completed main/control pairs are byte-identical, with 0U/0Cu/−17mask, confirming the switch-disabled control. Retain mask counts without attributing identical-output changes to routing.

| Control→guidance population | Completed boards | Fully connected | Median peak RSS MiB | Maximum peak RSS MiB |
|---|---:|---:|---:|---:|
| PCBench | 710→718 | 590→601 | 12.1→12.1 | 373.9→368.7 |
| Local | 10→10 | 8→7 | 13.6→13.5 | 72.2→121.4 |

The largest copper regression is **OpenHardwareExG Shield: 1→3 unrouted, 30→54 copper**. Control has 30 track-width violations; guidance has 34 width, 12 shorting-item and eight clearance violations. Short reports include AGND versus Net-(C30-Pad1) near C30. Its original is fully connected and has zero routing DRC, with 147 vias/2280.31mm. This is an ordinary failure, not an outlier exemption. Outside this board the completed total would be −47U/−2Cu, but it remains in all headline figures. The next investigation is the source of these new shorts; changing routing costs must not allow illegal copper.

Azalea remains a connectivity regression, 41→45U, but now has zero copper violations in both candidates after the merged pad/reference fixes. Motorizedopener remains 58→61U/53→54Cu. These are retained ordinary regressions.

Fresh clean workspace verification on the actual updated PR branch: **2,592 passed, 77 ignored, zero failed**, including doctests. All 600 crate/source/data/manifests match the measured frozen snapshot. Original JVM recordings are unchanged. The generated benchmark gate fails **107 quality losses**; [full summary](routing-quality-artifacts/corridor-current-main/pr-summary.md), [main comparison](routing-quality-artifacts/corridor-current-main/vs-main.json), [same-binary comparison](routing-quality-artifacts/corridor-current-main/vs-control.json), [output identities](routing-quality-artifacts/corridor-current-main/identities.json), and [source hashes](routing-quality-artifacts/corridor-current-main/source-hashes.json) are retained. Equivalent grouping/quality on the native web import path has not been validated.

The terminal-envelope extension is excluded: its seven-board completed pilot was +3U/−3Cu, held after four connectivity regressions and two improvements. The separate capacity audit found old/new metadata produce the same azalea /RESET band; endpoint span dominates, so no capacity patch was justified.

## Historical full comparison with project minimums

`quality-epyc-corridor-minimum-full-01`: main e9d10c2 and the combined candidate, guidance disabled/enabled in the same executable. Each has751 boards and successful KiCad referees (2,253 total); failed checks were repaired on unchanged SES. Both control and enabled candidate receive identical minimal project files containing original min_clearance and original PR25 pad-mask sidecars. Ten passes,300-second cap,one routing thread,192jobs. Results copied locally and to workbench.

| Group vs control | Unrouted delta / better / worse | Copper delta / gainers | Reported mask delta / gainers | CPU ratio |
|---|---:|---:|---:|---:|
| PCBench740 | −172 /44 /13 | −3 /13 | −64 /12 |0.9401|
| Local11 | +4 /0 /2 | +1 /1 |0 /0 |1.0078|

Of721 both-completed pairs,495 SES files are identical. The226 changed outputs jointly have **−46 unrouted/−3 copper/−64 reported mask**; the identical outputs have−13 reported mask, a referee difference not caused by routing. The30 deadline-affected pairs contribute−122 unrouted/+1 copper/+13mask. Thus headline−168 unrouted/−2 copper includes substantial deadline effects; no speed improvement is claimed.

Snappi is fully connected with **zero copper violations on both sides**. Before enforcing its project minimum, corridor guidance raised it from3 to10 copper violations. Ordinary regressions persist: azalea37→42U/45→51Cu; motorizedopener58→61U/53→54Cu. These are not outlier exemptions. All boards remain included.

| Group | Completed | Connected | Median RSS MB | Max RSS MB |
|---|---:|---:|---:|---:|
| PCBench |711→717|588→598|12.1→12.1|370.9→369.4|
| Local |10→10|8→7|12.1→12.1|69.8→118.8|

Versus main, the stack includes PR25/PR28 and dependencies: PCBench−939U/−84Cu/−4202reportedmask,151 connectivity improvements/33 regressions,21 copper gainers,CPU0.8704; local−11U/−1Cu/0mask,3 improvements/0 connectivity regressions,1 copper gainer,CPU0.9323. The [generated summary](routing-quality-artifacts/corridor-minimum/pr-summary.md) reports a failing262-quality-loss gate. [Incremental results](routing-quality-artifacts/corridor-minimum/vs-control.json), [main comparison](routing-quality-artifacts/corridor-minimum/vs-main.json), [output identities](routing-quality-artifacts/corridor-minimum/identities.json), and [frozen source hashes](routing-quality-artifacts/corridor-minimum/source-hashes.json) are retained.

The combined snapshot passed a clean workspace suite:2,586 passed/77ignored/0failed, including five corridor and two minimum-clearance tests. Seven production files in the merged review worktree match the frozen tested snapshot exactly. A fresh clean review-worktree suite also completed successfully: 2,586 passed, 77 ignored, zero failed, including doctests. Correct compilation paths and the new test names were verified. The implementation and earlier experiment evidence follow; earlier numbers below predate the project-minimum fix.

## Algorithm and historical experiment

The algorithm groups component pairs sharing at least four signals, excluding plane nets and nets spanning more than six components. It sorts endpoint projections and selects monotonic destination-order chains. Groups claim nets in a common deterministic order so a signal cannot count toward multiple corridors. A finite band covers the selected endpoints with width allowing the estimated trace and clearance capacity. Physical maze movement outside the band receives a 25% additive cost proportional to the segment fraction outside. Existing geometric options remain available. Destination heuristics and drill-page lower bounds are unchanged; physical door/drill movement costs include the preference. Optimizer searches also use it. This does not add bus queue ordering or reserve hard lanes.

## Full experiment

`quality-epyc-corridor-full-01`, frozen main e9d10c2 and c461555 with guidance disabled/enabled,751 boards each,192 concurrent single-thread jobs, ten passes,300-second routing limit. All 2,253 cells have successful KiCad referees; failed referee processes were repaired on unchanged SES files. No Java DRC scoring. CPU figures are descriptive single-run ratios, not a timing claim. Mask report caps limit exact interpretation on heavily violating boards. All boards remain in the headline.

| Group / comparison | U delta | U improved / regressed | Copper delta / gainers | Reported mask delta / gainers | CPU ratio |
|---|---:|---:|---:|---:|---:|
| PCBench740 vs control | −245 |45 /14|−1 /14|−113 /13|0.9418|
| Local11 vs control |+4|0 /2|+1 /1|0 /0|1.0058|
| PCBench740 vs main |−1034|154 /34|−52 /23|−4169 /12|0.8696|
| Local11 vs main |−11|3 /0|−1 /1|0 /0|0.9236|

Both groups use KiCad. Compared with main, the combined candidate includes PR25 and its dependencies; those gains must not all be attributed to this algorithm. The incremental comparison is guidance versus control.

| Group vs control | Completed boards | Fully connected boards | Median RSS MB | Maximum RSS MB |
|---|---:|---:|---:|---:|
| PCBench740 |711→717|588→598|12.1→12.1|369.4→369.4|
| Local11 |10→10|8→7|13.6→12.0|69.4→120.9|

Among 721 pairs where both routers completed, deltas are−46U/−1 copper/−122 reported mask. The 30 deadline-affected pairs contribute −195U/+1 copper/+9 reported mask. Completion includes the router's internal final state, rather than relying solely on the outer timeout flag.

## Regressions and original-board evidence

The 300-second decelerator4030 result worsens450→493U and 10→11 copper; both time out. Its original is connected, with 206 nets,865 vias and28,322mm of copper over two layers. A matched600-second diagnostic reaches 316U/7 copper in control and 312U/6 copper with guidance, both still timed out after three passes. This supports deadline sensitivity, not an outlier exemption or replacement for the headline.

Azalea reproducibly worsens 37→42U and45→51 copper; the local motorized opener worsens 58→61U and53→54 copper, both completing ten passes. These remain counted regressions. The motorized-opener fixture has no original routed copper or vias; its saved original KiCad report already contains37 clearance violations and149 unconnected entries. It provides no human-routed solution for comparison, and these input issues do not explain away the candidate’s additional losses. Azalea's KiCad errors demand local pad clearance absent from DSN; a separate import fix is being measured, and is not included here.

Snappi-zero remains fully connected but increases 3→10 copper violations. The original project minimum is203µm while DSN includes a152.4µm net-class clearance. A controlled input correction, with the same corridor binary, eliminates all 10 copper violations without losing connectivity. This diagnoses missing input rules, not a waiver; the unchanged headline still includes the violations. No registered artwork outlier explains these ordinary losses.

Selected-board geometry pilots keep NRC2016 and PocketBone fully connected with unchanged copper/mask totals. NRC2016 length 4009.74→3947.53mm with 25 vias; PocketBone1658.30→1596.01mm and83→75 vias. SNAP is unchanged. GB-CART improves 11→9U but gains one copper violation, so its shorter incomplete output is not an efficiency claim.

## Validation and review status

Clean `cargo test --workspace --no-fail-fast`: **2,584 passed, 77 ignored, zero failed**, including doctests. All workspace crates were rebuilt from this worktree, and all five corridor tests ran. Five corridor tests cover exact band clipping, subdivision/reversal consistency, rotation/range, monotonic chain selection and consistent ownership across overlapping groups. Regression tests were observed failing before their implementations. Original JVM recordings are unchanged. The environment switch leaves existing behavior as default; enabled behavior is validated by the full corpus, not inferred from default tests.

The frozen source hashes are retained. The review source differs only by rustfmt ordering the control/corridor module declarations; this has no intended routing behavior effect. [Hashes](routing-quality-artifacts/corridor-coherent/source-sha256.txt), [incremental results](routing-quality-artifacts/corridor-coherent/vs-control.json), [main comparison](routing-quality-artifacts/corridor-coherent/vs-main.json) and [benchmark PR summary](routing-quality-artifacts/corridor-coherent/pr-summary.md) are retained. The benchmark gate reports 264 quality losses versus main, including score components beyond connection and DRC totals. Draft review is appropriate; this is not a claim that the gate passes or that default activation is ready.

The measurements use DSN component identities and PR25's KiCad-derived pad-mask metadata. The web adapter currently represents each pad as a separate component to preserve pad rotation, which does not retain the component grouping required here. This prototype does not claim equivalent bus guidance on that native web import path.
