# Preserve explicit DSN image identity

Numbered DSN images can have different pin names and pad geometry. Import previously stripped their numeric suffixes and reassigned names by insertion order. A placement could therefore receive another image's pads. This change retains explicit image names and prefers an exact opposite-side image before the existing unsuffixed fallback. All existing fallback options remain available.

On saiboard_8x3, SOT-23::1 uses 0.9×0.8mm rectangular pads while SOT-23::25 uses 1.475×0.6mm rounded rectangular pads. The old import displaced Q3 pin1 by 62.5µm relative to the original KiCad board; corrected import matches the original. An unchanged-session diagnostic previously returned no internal DRC errors with wrong geometry; corrected geometry detects 13 clearance violations and three shorts. That diagnostic demonstrates checker sensitivity, not a routing improvement or one-to-one KiCad count parity.

## Final validation after PR29 merged

`quality-epyc-image-pad-full-01` compares `pads` (equivalent to current main **028f0a5**) with `combined` (main plus the image fix). Both receive the same verified local pad/reference metadata and project minimum rules. Candidate labels retain the earlier snapshot names; source/crate-data hashes prove the baseline matches current main and the candidate matches this integrated worktree. All **2,253 KiCad referee results succeeded** across three candidates; the additional `main` candidate is historical d84ac9e. Each candidate routed 751 boards, ten passes, 300-second cap, one routing thread, 192 jobs.

| Group vs current main | Unrouted delta / improved / regressed boards | Copper delta / improved / gainers | Reported mask delta / gainers | CPU ratio |
|---|---:|---:|---:|---:|
| PCBench 740 | −288 / 25 / 4 | −46 / 10 / 3 | −766 / 7 | 0.93458 |
| Local 11 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / 0 | 1.00082 |

**The 720 pairs completed by both versions give −15 unrouted connections and −46 copper violations.** Deadline effects account for the remaining apparent 273-connection improvement. No 288-connection or 6.5% speed claim. Of these completed pairs, 186 SES outputs are identical (zero U/copper difference, −723 reported mask); 534 differ (−15 U/−46 copper/−49 mask). Image-name changes can alter SES text without changing tracks. KiCad's variable mask counts on identical output prevent claiming the large raw mask reduction as a routing gain.

PCBench completed boards rise 710→718, connected boards 588→590, median RSS stays 12.1MB, maximum RSS 370.9→372.7MB, CPU 34772.96→32498.04s. Local completed/connected stay 10/8; median RSS 13.6→12.1MB, maximum 74.9→78.0MB, CPU 782.41→783.05s.

Four completed boards gain one unrouted connection each: minimal_node_rfm69w, Pi1541io, teensy-fx and esp32-ethernet. Esp32 also gains one copper violation; balena-rover gains one copper violation with no connection loss. All remain ordinary regressions. Serial_gw's earlier +2 connection regression is resolved when combined with the landed local pad rules. Chess adds two copper violations in a deadline-affected pair. The two completed copper gainers still involve local NPTH rules omitted when mechanical pads become DSN keepouts; a separate prototype is under measurement. No outlier exemption is used.

The [generated comparison](routing-quality-artifacts/image-identity-main/post-pad/pr-summary.md) fails its aggregate gate with **17 routing-quality losses**. The completed-board improvements support merging under the accepted trade-off policy, with all losses explicit. [Full results](routing-quality-artifacts/image-identity-main/post-pad/vs-main.json) and [identity audit](routing-quality-artifacts/image-identity-main/post-pad/identities.json) are retained and backed up locally/workbench.

Fresh post-merge workspace validation passed **2,587 tests, 77 ignored, zero failures**, including doctests. Production and test sources match the measured combined snapshot. Only the experiment log required merge-conflict resolution; both histories were retained. Original JVM recordings remain unchanged; the deliberate image-identity parity expectations described below remain in place.

## Earlier benchmark against main d84ac9e

`quality-epyc-image-main-01`: exact main d84ac9e versus main plus the two production changes, 751 boards each, **all 1,502 KiCad referees successful**. Settings: ten passes, 300 seconds, one routing thread, 192 jobs. Both sides use identical pad-mask metadata and project minimum-clearance files. PR29 local copper floors and the new reference remapping are not included. Main was rechecked unchanged after measurement. Results are copied locally and to workbench.

| Group | Unrouted delta / better / worse boards | Copper delta / better / gainers | Reported mask delta / gainers | CPU ratio |
|---|---:|---:|---:|---:|
| PCBench 740 | −346 / 25 / 6 | −49 / 11 / 3 | −15 / 7 | 0.93829 |
| Local 11 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / 0 | 1.00626 |

**The 720 boards completed by both versions give −11 unrouted and −48 copper violations.** The 31 deadline-affected pairs account for −335 unrouted and −1 copper. The raw 346-connection gain and CPU ratio do not establish a corresponding causal improvement or speedup.

Of the 720 completed pairs, 185 SES outputs are byte-identical (zero connection/copper difference, +4 reported mask); 535 differ (−11 unrouted/−48 copper/−18 reported mask). Explicit image names change SES placement text, so a changed session does not necessarily mean changed wire geometry. KiCad mask counts can vary on fixed output; retain them without claiming an established mask gain.

| Group | Completed | Unfinished | Fully connected | Median RSS MB | Maximum RSS MB | Total CPU seconds |
|---|---:|---:|---:|---:|---:|---:|
| PCBench | 710→718 | 30→22 | 588→590 | 12.1→12.1 | 369.4→373.9 | 34707.31→32565.62 |
| Local | 10→10 | 1→1 | 8→8 | 12.1→12.1 | 71.7→70.3 | 779.85→784.73 |

## Regressions

| Completed board | Unrouted delta | Copper delta |
|---|---:|---:|
| Hardware_Playground_serial_gw_ATMEGA328P | +2 | 0 |
| Hardware_Playground_minimal_node_rfm69w | +2 | −2 |
| Pi1541io_Pi1541io | +3 | 0 |
| kitspace_teensy-fx | +1 | 0 |
| esp32-ethernet_esp32-ethernet | +1 | +1 |
| balena-rover-wide-hat_resin-rover | 0 | +1 |

These remain ordinary regressions; no outlier exemption or board exclusion is used. The two completed copper gainers involve local NPTH-pad clearance: esp32 has two rather than one error against a 1.650mm pad rule; balena has three rather than two against 1.725mm rules. Both originals have zero routing DRC and zero unconnected items. The local-rule and metadata experiments may help, but their interaction with this image fix is not measured yet. Deadline-affected chess adds three copper errors and bms adds one unrouted connection. Saiboard_8x3 loses 17 copper errors while retaining zero unrouted connections.

The [generated benchmark summary](routing-quality-artifacts/image-identity-main/pr-summary.md) reports a **failed gate with 19 routing-quality losses**. The positive aggregate supports review, not a no-regression claim. Historical image preservation on an older baseline was negative (+71 unrouted/−25 copper across 739 KiCad boards); this is a new measurement against main with the landed routing fixes, not a reinterpretation of that earlier result.

## Tests and deliberate Java divergence

A failing-first image-library test reproduced loss of explicit names and pad geometry; it passes with the fix. A package lookup test covers exact opposite-side preference and retention of the absent-variant fallback. A clean full workspace run passed **2,582 tests, 77 ignored, zero failed**, including doctests. The two production source hashes match the frozen benchmark snapshot.

Five Java-parity tests required explicit review. Original JVM recordings remain unchanged. BBD import item IDs and tree ordering change when distinct image definitions survive import; all non-order override transcript fields were independently verified unchanged before adding four Rust tree-order expectations. Three renumbered pin identities are independently asserted as U102-17, U102-23 and C2-1. Relay import expectations account for distinct image outlines and names. SES tests retain JVM wiring and placement fields while checking image names against the DSN, with the pre-existing unquoted Cyrillic lexer exception retained. These are intended input-correctness divergences, not silent re-recording.

[Full comparison](routing-quality-artifacts/image-identity-main/vs-main.json), [output identities](routing-quality-artifacts/image-identity-main/identities.json), original-board regression audit and diagnostic artifacts are retained alongside this report.
