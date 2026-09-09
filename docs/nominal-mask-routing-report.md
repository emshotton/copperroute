# Nominal clearance with pad-mask routing guards

This candidate combines nominal clearance queries, a minimum two-unit smoothing reserve, an additional dogleg escape, and pad-specific mask clearance floors capped by the existing pad-to-pad gap. It includes the via-name fix from PR24 and depends on the separate native mask checker PR22. The connection-check cache is already on main. Failed-insertion recovery and bus-order experiments are not included.

The original full 751-board prototype comparison had 1,012 fewer unrouted connections, 57 fewer copper DRC violations, and 4,382 fewer reported mask violations. Connectivity improved on 142 boards and regressed on 39; copper reports increased on 27 boards. These are aggregate benefits with real ordinary-board regressions, not a clean regression gate.

## Outlier-aware review

Two documented artwork cases accounted for 70 of 87 added mask reports (80.5%):

| Board | Main mask | Prototype mask | Added reports | Evidence |
|---|---:|---:|---:|---|
| `pcbench-8bit-cpu_programming_interface` | 0 | 34 | 34 | Stripping removes 223 unnetted copper artwork tracks but retains 223 mask apertures. |
| `pcbench-FRM16_Relay_Module_I2C_Controller_relay_controller` | 5 | 41 | 36 | Independent B.Mask text and polygon artwork is outside the pad-based routing model. |

These are valid physical features. Their unusual representation helps explain the losses; it does not waive KiCad's findings or remove boards from the benchmark. See [the evidence register](outlier-boards.md). Under the requested review policy this concentration warrants considering the broad improvement rather than rejecting it solely for these artwork losses.

Ordinary losses remain: excluding all three registered outliers still leaves 38 connectivity regressions and 26 copper-DRC gainers in that run. GB-CART256K-A worsens3→11 unrouted and is **not** classified as an outlier. A 73-board diagnostic ablation reproduces that loss with nominal clearance alone; smoothing alone preserves3 unrouted. Dorkyboard and rxadc each regress0→4. These must be weighed alongside the gains.

## Reproduction and limits

KiCad 10.0.4 is the only referee. Each full comparison uses 740 PCBench boards plus 11 KiCad fixtures, ten passes, a 300-second routing limit, one router thread, and 192 concurrent jobs. No Java-scored boards enter these totals. CPU ratios describe a single run; no timing improvement is claimed. Deadline-limited boards and KiCad's capped report counts can vary, including on identical SES output.

DSN lacks mask expansion metadata. The benchmark supplies `COPPERROUTE_PAD_CLEARANCE_JSON`, an array of `{component,pad,x_um,y_um,front_um,back_um}` records extracted from the original KiCad input. Coordinates and clearances are micrometres. The loader matches component/pad position (or an unambiguous pad name), raises clearance floors, and caps each requested mask floor at the existing foreign-pad gap. It never lowers the existing copper rule. Invalid JSON fails loading; unmatched records warn. Native KiCad board JSON supplies mask expansion and constraints directly through PR22. The DSN benchmark result therefore requires the metadata sidecar; it is not a claim about unannotated DSN inputs.

## Validation

Original JVM recordings are retained. Explicit nominal-clearance expectations document intentional geometric divergences. Independent tests exercise nominal boundary acceptance, minimum smoothing reserve, pad clearance preservation, dogleg escape, and wrapper/direct-pipeline equivalence. The fanout no-op regression retains the same positive sub-grid movement by extending the query target so the obstacle intersects the nominal search envelope.

The packaged production sources match the frozen full-run snapshot after formatting; the only semantic source-file difference after freezing is a test-only nominal-width expectation. The production-equivalence audit is retained alongside the source manifest. `cargo test --workspace --no-fail-fast`: **2,579 passed, 77 ignored, zero failures**. Changed Rust files pass rustfmt; the whole-tree formatting check also exposes an unrelated pre-existing formatting difference in `kicad_browser_geometry.rs`, which is left untouched.

## Packaged full-corpus result

Run `quality-epyc-mask-review-01` compares latest main `e9d10c21da58653fb5332df44021f25180a3d4da` with the immutable review snapshot based on checker PR22 `36283c2`. All 751 referee results per candidate are successful. 68 initial referee failures were rescored on unchanged SES output, with initial reports retained; no boards were rerouted for this repair.

| Group | U main → review | U improved / regressed | Copper main → review | Copper gainers | Mask main → review | Mask gainers | CPU ratio |
|---|---:|---:|---:|---:|---:|---:|---:|
| PCBench (740; KiCad) | 5282 → 4132 | 140 / 38 | 933 → 885 | 26 | 15193 → 10949 | 11 | 0.8730 |
| Local fixtures (11; KiCad) | 241 → 226 | 3 / 0 | 59 → 57 | 1 | 0 → 0 | 0 | 0.9189 |

Combined: **1,165 fewer unrouted, 50 fewer copper DRC, and 4,244 fewer reported mask violations**; 143 connectivity improvements and 38 regressions. The automatic gate **fails with 268 routing-quality losses**, including other score components. Full [generated benchmark summary](routing-quality-artifacts/mask-routing/bench-pr-summary.md) and [per-board deltas](routing-quality-artifacts/mask-routing/review-comparison.json) are retained.

The two artwork boards contribute 68 of 112 added mask reports (60.7%) in this repeat: programming-interface +32, relay-controller +36. The earlier 70/87 result and this repeat both concentrate the majority of added mask reports on these two boards. Report caps and deadline effects prevent treating the exact share as fixed. Ordinary losses remain; this is a reason to review the broad benefit, not a claim that every loss is explained by outliers.

The packaged candidate and original prototype produced **identical SES on all 720 boards where both completed** (727 identical overall). All differences lie among pairs with at least one timeout. Production packaging therefore preserved the observed completed routing behavior.

| Metric | Main | Review |
|---|---:|---:|
| Routing completed | 721 | 729 |
| Fully connected (KiCad) | 527 | 596 |
| Total CPU seconds | 38308.73 | 33484.24 |
| PCBench median peak RSS (MB) | 13.6 | 12.1 |
| PCBench maximum peak RSS (MB) | 450.4 | 370.9 |

Full raw results are mirrored to `/home/em/copperroute-epyc-results` on workbench; reports and exports are also copied to the laptop.
