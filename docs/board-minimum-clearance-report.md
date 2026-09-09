# Project minimum copper clearance

The router loads KiCad project minimum clearance for DRC but previously did not apply it to routing clearance classes. On Snappi-zero, DSN permits 152.4µm while the project requires 203µm. On mini_ice 40, the imported route rules likewise permit tracks that KiCad rejects. Board preparation now raises copper-pair clearance requirements to the supplied project minimum and refreshes cached search shapes.

It collects classes used by existing copper items and configured future traces, vias, pads and conduction areas. Higher values remain unchanged. Class 0 and dedicated non-copper classes are preserved. A non-copper obstacle sharing a copper class can conservatively inherit that class's increased value; the matrix cannot distinguish item types within one class. This is not full KiCad custom-rule precedence. A project must be supplied, for example `copperroute route in.dsn --kicad-project board.kicad_pro -o out.ses`; DSN alone does not recover an omitted board minimum.

## Evidence against the matching control

`quality-epyc-board-minimum-full-01`: main e 9d 10c 2, c 461555 control and c 461555 plus this change; 751 each, 192 concurrent single-thread jobs,ten passes, 300-second cap. All 2253 cells have successful KiCad referees, including repairs on unchanged SES. Both control and candidate receive identical minimal project files containing only original min_clearance, plus the same PR #25 pad-mask sidecars. No Java DRC scoring. Source and binaries are frozen; no build overlaps measurements.

The most direct evidence is the 720 both-completed pairs.717 SES files are byte-identical. The only changed outputs are:

| Board | Unrouted | Copper DRC |
|---|---:|---:|
| Snappi-zero |0→0|3→0|
| CATs-Eurosynth LFO_Main |4→3|1→0|
| mini_ice 40 |0→0|27→0|

Thus completed outputs improve by one connection and 31 copper violations, with no connectivity/copper regression. Their reported mask total differs by +10 entirely on identical SES files; this is not a routing change. These are ordinary design rules, not outlier exclusions.

| Group vs control | U delta / better / worse | Copper delta / gainers | Reported mask delta / gainers | CPU ratio |
|---|---:|---:|---:|---:|
| PCBench 740 |−336 /16 /0|−29 /1|−1 /6|0.9444|
| Local 11 |0 /0 /0|0 /0|0 /0|0.9998|

The 31 deadline-affected pairs contribute −335U/+2 copper/−11 reported mask. Most connection gains therefore cannot be attributed confidently to this rule fix or treated as a speed claim. The one copper-gaining board is real-time-chess_kfchess:−7U/+3 copper, deadline-affected. All boards remain included. One repetition does not establish timing improvement, and capped/variable mask reports are disclosed.

| Group vs control | Completed | Connected | Median RSS MB | Max RSS MB |
|---|---:|---:|---:|---:|
| PCBench |710→717|588→589|12.1→12.1|369.4→370.9|
| Local |10→10|8→8|13.6→12.1|71.3→72.8|

## Comparison with main and validation

The main comparison includes PR #25 and its dependencies; its improvements are not all caused by this change. PCBench delta −1094U/−79 copper/−4264 reported mask, 140 connectivity improvements/38 regressions, 24 copper gainers,CPU ratio 0.8734. Local delta −15U/−2 copper/0mask, 3 improvements/0 connectivity regressions, 1 copper gainer,CPU ratio 0.9211. The generated [benchmark PR summary](routing-quality-artifacts/board-minimum/pr-summary.md) includes the failing aggregate gate. Incremental [control comparison](routing-quality-artifacts/board-minimum/vs-control.json), [main comparison](routing-quality-artifacts/board-minimum/vs-main.json) and [completed-output hashes](routing-quality-artifacts/board-minimum/identities.json) are retained.

The six-board pilot independently had −1U/−31 copper/−1 reported mask and no quality regressions; CPU ratio 1.0535. This supports the rule mechanism without establishing a speed effect.

Two tests cover minimum flooring without reducing higher/null/dedicated rules, idempotence and refreshing compensated pin shapes. The matrix test failed before implementation; the shape test fails if cache refresh is disabled. JVM recordings are unchanged. A clean explicit-manifest workspace run completed successfully: 2, 581 passed, 77 ignored, zero failures, including doctests. Compilation paths and both new test names were verified to exclude stale shared-target artifacts.
