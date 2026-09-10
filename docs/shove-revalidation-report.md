# Revalidate a path after sequential shoves

A later shove can move another net's copper into a segment already checked earlier in the same forced-polyline insertion. On OpenHardwareExG Shield with corridor guidance, shove step9 puts AGND across signal segments1–3. The forced insertion reports its endpoint as reached and inserts overlapping copper; the overall connection fails later. The original designer board has zero unrouted/copper DRC, so this is not an outlier exemption.

A captured pre-insertion board imports without warnings and reproduces the short in under two seconds. All observer-enabled, observer-disabled and original scored sessions are byte-identical. The regression fails before the prototype (one short where zero is required), then passes when a final non-shoving check rejects the invalid path.

The prototype retains the original checks and adds a final check for multi-segment paths when `with_check` is set. It uses the same clearance and entry-side rules, with trace/via/spring recursion set to zero, before inserting copper. Single-segment and explicitly unchecked calls keep their existing path. Enable with `COPPERROUTE_RECHECK_SHOVED_PATH=1` for measurement. No geometry snapshots, diagnostics or new production dependencies are included.

This adds work and can change later routing after rejection. Connectivity and performance effects require the full corpus comparison; the unit regression alone does not establish mergeability. A five-candidate full run against main54d4e80 is complete; results are below. The candidate combines opt-in corridor code solely to measure each feature alone and together. The first clean workspace run passed 2,597 top-level tests plus one isolated guard child, 77 ignored, zero failed. A complementary test verifies a clear three-point prefix still reaches its requested endpoint; both guard tests pass. The final workspace run including that added case passed 2,598 top-level tests plus two isolated child checks, 77 ignored, zero failed. The raw harness total is 2,600 because it includes the two subprocess checks alongside their parent tests. Evidence: `routing-quality-artifacts/shove-revalidation/workspace-summary.txt`. Acceptance remains pending regression review.

Full-board targeted result from the ongoing full run: all five OpenHardwareExG Shield cases completed with successful KiCad referees. Main/control/guard: 1U, 30 copper violations (track width). Corridor: 3U, 54 copper (12 shorts, 8 clearance, 34 width). Corridor+guard: 3U, 34 copper (all width). Thus the final path guard removes all 20 new shorts/clearance violations without changing the corridor result's unrouted count. Relative to main, +2U/+4 width violations remain counted on this ordinary board. Full corpus acceptance remains pending. Raw SES/metrics/KiCad reports for all five cases are preserved locally in shove-revalidation-shield-diagnostic.tar.gz and mirrored through the server reports backup.

## Full KiCad result against main 54d4e80

quality-epyc-shove-revalidation-full-01 completed all 3,755 cases: five variants × 751 boards, 10 passes, 300-second cap, 192 single-thread workers. Every referee status is ok after rescoring failed referees on unchanged SES outputs. Results are preserved on the laptop and mirrored to workbench.

| Comparison | Group | Raw delta U | U better/worse | Raw delta copper | Copper gainers | CPU ratio | Completed pairs | Paired U / copper |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| main-vs-guard | pcbench | -1 | 6/2 | +1 | 1 | 1.0013 | 709 | +6 / +0 |
| main-vs-guard | local | +0 | 0/0 | +0 | 0 | 1.0062 | 10 | +0 / +0 |
| main-vs-corridor | pcbench | -47 | 39/18 | +17 | 8 | 1.0006 | 709 | -47 / +20 |
| main-vs-corridor | local | +4 | 0/2 | +1 | 1 | 1.0142 | 10 | +3 / +1 |
| main-vs-corridor-guard | pcbench | -282 | 45/16 | -1 | 8 | 0.9432 | 709 | -39 / -1 |
| main-vs-corridor-guard | local | +4 | 0/2 | +1 | 1 | 1.0194 | 10 | +3 / +1 |
| corridor-vs-corridor-guard | pcbench | -235 | 14/4 | -18 | 3 | 0.9426 | 713 | +8 / -21 |
| corridor-vs-corridor-guard | local | +0 | 0/0 | +0 | 0 | 1.0051 | 10 | +0 / +0 |

Combined versus main on 719 completed pairs: **−36 unrouted / 0 net copper**, with U improving on 29 boards and worsening on 13; copper improves on seven and worsens on seven. PCBench contributes −39U/−1 copper; local KiCad fixtures +3U/+1 copper. No board is excluded as an outlier. Main/control completed SES files are all byte-identical and have 0U/0 copper delta.

The guard versus corridor removes 21 copper violations on completed pairs but adds eight unrouted connections: Shield −20 copper, GB-CART256K-A −1U/−1 copper, boatcontrol_NonLatchingNO30A +6U and uSKY +3U. The latter two regressions need investigation. Guard alone gives +6U/0 copper on completed pairs, so there is no demonstrated standalone aggregate gain on main.

For combined versus main, 492 completed SES outputs are identical (0U/0 copper/−7 mask), while 227 differ (−36U/0 copper/−91 mask). Raw mask counts are retained; identical-output differences cannot be attributed to the routing algorithm. Raw timeout totals and single-run CPU ratios are observations, not causal speed claims.

PCBench main/combined completed counts are 709/717, fully connected 589/599, median RSS12.1/12.1MiB, max372.4/375.4MiB. Local completed10/10, fully connected8/7, medianRSS12.0/13.5MiB, max75.8/105.1MiB. The completed-pair view excludes either deadline/non-COMPLETED outcome.

The generated benchmark gate fails: guard six quality losses, corridor112, combined112. The full generated report is retained in routing-quality-artifacts/shove-revalidation/pr-summary.md; this is not a clean gate. Candidate remains uncommitted pending regression review.

Capture replay completed successfully; captures import with no warnings. uSKY route net10=GND: same-net trace185 is not an obstacle, but trace184 on net11 is an obstacle. Boatcontrol route net27=Net-(J102-Pad10): trace914 on net1 is an obstacle; same-net trace915 and pin87 are not. Both direct shape checks reject. This refutes treating the reported same-net obstacle as permission to bypass clearance; no relaxation implemented. Capture observer-on/off/frozen guard SES hashes are identical for both boards; all four KiCad scores are successful and completed.

Combined copper-gainer audit (all retained): Shield +4 track-width; VC4000 +2 shorts against B.Cu text; rp2040-dmxsun baseboard2slots +3 starved thermals; Patternflow +1 starved thermal; ESP07 +1 clearance; pmw3360_jst +1 copper-edge clearance; motorizedopener +1 hole clearance. VC4000 original has0U/0routingDRC; KiCad names text VC4000 MultiROM v0.4 / Keller/Maibaum in every reported short. DSN has no text string, keepout or wire; all13polygon declarations are footprint outlines. This is a confirmed copper-text input omission, analogous to AVR-fuser, not an excuse to drop its +2Cu/−2U contribution.

## Coupled guidance safety validation

Prepared guidance so COPPERROUTE_CORRIDOR_GUIDANCE automatically enables the final path guard. The standalone experimental recheck switch remains available; the guidance switch stays opt-in. This prevents enabling the structural routing experiment without its required safety check. New regression corridor_guidance_also_revalidates_paths failed first: expected original corner but reached804926/−701946 with guidance alone. After sharing the guidance-enabled predicate with the final guard, all three guard tests pass. No JVM recordings changed.

Final workspace suite35303 running against this source. EPYC driver991239 starts quality-epyc-shove-coupled-full-01, main/control/coupled,751KiCadboards each,10passes300s1thread192jobs.602source/data/test/fixture hashes match local before launch. Uses the same merged local-hole metadata01 and project floors. Failed referees rescore without rerouting before exports. New-source acceptance and final workspace completion pending; prior five-way result remains an experiment result, not a claim that this new source has completed qualification.

Final coupled-source workspace verification completed (session35303 terminal0): 2,599 top-level tests plus three isolated guard checks passed,77ignored0failed. Raw harness sum2602 includes children. Evidence: coupled-workspace-summary.txt and preserved full local log. Corpus991239/postprocess1312504 remain live; no final quality acceptance claim.

Fresh actual PR27-branch workspace suite12447 completed terminal0:2599top-level passed plus3childchecks,77ignored0failed. All602source/data files reverified against frozen coupled source after the suite. Full log preserved locally; pr27-workspace-summary.txt retained. Merge remains uncommitted pending corpus991239 and postprocess1312504, both verified live after final board submissions.
