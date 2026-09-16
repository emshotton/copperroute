# Group planning subset experiment — 2026-09-13

Base: `66612acf`. 16 purposively selected boards (12 PCBench, four local fixtures), all scored by KiCad 10.0.4. Ten passes, 600-second deadline, one router thread, twelve jobs on workbench. This is an exploratory screen, not a full-corpus validation.

Positive deltas mean worse. Copper/routing and solder-mask findings are shown separately; both matter. CPU ratios include different routing work and deadline effects. Some boards reach the mask report cap; uncapped pairs are reported separately.

## Interpretation

All 64 cells completed evaluation with valid KiCad scores. These are bounded prototypes, not full implementations of polygon/MCTS global routing or an optimal CBS solver. Neither is enabled by default or being proposed for merge from this screen.

The rebuild control attempted 41 groups: 12 failed during independent routing, 27 had conflicts between their independently completed nets, and two were solved and retained. Topological alternatives retained three groups, adding only HBR beyond that control. Conflict search retained six, adding SRAMBO, Mouse, TK44 and GB-CARTPP-XC. No prototype panicked. Source fingerprints match the remote candidate, full workspace tests passed, and all 13 non-timeout baseline comparisons available from the prior screen match unrouted/copper counts.

On 14 pairs that completed without a deadline on both sides, rebuild changes no quality counts, topological alternatives add two missing connections, and conflict search removes three missing connections with unchanged copper counts but 29 additional mask bridges on SRAMBO. This is the strongest comparison of the added search mechanisms.

Nine of the topological/conflict headline connection gains come from Karabas (39→30). Neither retained any group repair there, so those deadline-sensitive differences are not credited as proof of algorithm improvement. Chess (67→65 missing, copper 70→73) changes identically in all three experimental arms and is therefore a shared rebuild effect plus a deadline-affected comparison, not a demonstrated benefit of either added search. Main hit the deadline on both chess and Karabas; all conflict-arm boards completed. CPU totals and ratios describe this run, not a proven speed improvement.

**Decision:** the conflict-based prototype is worth further investigation because it has three distinct, normally completed board gains and no normally completed connectivity regression in this screen. It is not merge-ready: SRAMBO's +29 uncapped mask errors and the shared chess copper regression remain material. The current obstacle-cut topological prototype shows no added final quality gain beyond the control and worsens HBR; park this particular implementation. That does not disprove broader topological/bus planning, which this small-group prototype does not implement. No full-corpus run was launched for these versions.

## Board-level investigation

- **SRAMBO:** conflict search changes KiCad incompletes 1→0 (closes GND), copper 0→0, mask bridges 136→165. Wirelength changes 3547.0889→3523.8716 mm; vias 92→58. The fully connected original is 3481.1571 mm/39 vias with zero routing DRC and 96 total original DRC findings. The candidate is closer to the reference's wire/via usage, but the additional mask errors are real and below the cap. Example candidate finding: F.Cu `/TDI` trace against P1 pad 3 `/TDO`. The original's existing findings are not an excuse for adding 29 errors.
- **Mouse:** 4→3 incompletes, copper 0→0, mask count 44→44. Two +5V disconnections close while GND changes 2→3. Wirelength 924.5609→925.2092 mm; vias 29→38. This is a net completion gain with a ground-connectivity trade, not unchanged individual DRC findings simply because the mask totals match.
- **GB-CARTPP-XC:** 9→8 incompletes, copper/mask unchanged at zero. GND changes 8→7 while `/SH` remains open. Wirelength 1151.6135→1180.9242 mm; vias 65→67. The complete original is 1394.4127 mm/144 vias; comparing incomplete lengths to the complete original does not establish efficiency.
- **HBR digital:** topological search changes 14→16 incompletes, all GND; copper/mask remain zero. Wirelength 2044.0578→2089.6572 mm, 23 vias in both. The original is completely routed and DRC-clean (2115.3198 mm/98 vias), so there is no evidence for dismissing this as a designer outlier.
- **TK44:** a conflict-free group was retained but final completeness/copper/mask totals did not improve. Solving a group does not necessarily improve the finished board.

### Original-board layout comparison

These KiCad copper plots omit filled zones for legibility; DRC measurements use the fully imported and refilled boards. The saved main SVG comes from an earlier run whose SRAMBO SES was verified byte-identical to this run's baseline. Visual inspection shows fewer extra layer changes in the conflict candidate than baseline, but the designer's longer bundles and pin escapes remain more orderly. This is evidence to pursue coordinated escape/bundle planning, not to equate this prototype with human layout quality.

- [Original SRAMBO copper](layouts/srambo-original-tracks-only.svg)
- [Baseline SRAMBO copper](layouts/srambo-main-tracks-only.svg)
- [Conflict-search SRAMBO copper](layouts/srambo-conflicts-tracks-only.svg)

### Why the internal acceptance test did not protect solder mask

`BoardStatistics` does call the full internal DRC, but `checks/solder_mask.rs` skips pins with no `solder_mask_expansion` metadata. Native KiCad import populates that metadata; ordinary DSN import does not carry the original pad mask apertures. This screen routes DSN plus project rules, while KiCad later scores the SES imported into the original footprint geometry. Missing mask geometry is therefore a concrete likely contributor to the acceptance gap. A paired native-KiCad-input experiment, or faithful pad-mask metadata transfer, is a justified next test; neither was tested here.

The incomplete-count acceptance also remains an imperfect proxy for ground connectivity after KiCad refills zones. HBR and Mouse show why complete signal groups do not guarantee improved whole-board external connectivity.

## Provenance and scope

Worktree `copperroute-group-planning`, branch `experiment/group-planning`, base/main `66612acf`. Large outputs remain at `/home/em/copperroute-group-artifacts` and `/home/em/copperroute-congestion-main/benchmark/results/group-planning-subset-01`. Small reports, diagnostics, source hashes, official exports and layouts are copied locally. All references are KiCad; no Java DRC fixtures were used.

The group planners are inspired by [polygon-based PCB routing](https://past.date-conference.com/proceedings-archive/2023/DATA/603.pdf) and [conflict-based search](https://ojs.aaai.org/index.php/SOCS/article/view/18222), with deliberately narrower implementations and no transferred theoretical guarantees.

| Variant / subset | Paired | Unrouted baseline → candidate (Δ) | U improved / regressed boards | Copper Δ / gaining boards | Mask Δ / gaining boards | CPU ratio | Fully connected baseline → candidate | Deadline baseline → candidate | Max RSS baseline → candidate (MiB) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| rebuild / all | 16 | 330 → 329 (-1) | 1 / 1 | +3 / 1 | +0 / 0 | 1.075× | 4 → 4 | 2 → 1 | 178.8 → 173.2 |
| rebuild / pcbench | 12 | 329 → 328 (-1) | 1 / 1 | +3 / 1 | +0 / 0 | 1.077× | 1 → 1 | 2 → 1 | 178.8 → 173.2 |
| rebuild / local | 4 | 1 → 1 (+0) | 0 / 0 | +0 / 0 | +0 / 0 | 1.062× | 3 → 3 | 0 → 0 | 50.4 → 51.2 |
| topology / all | 16 | 330 → 321 (-9) | 2 / 1 | +3 / 1 | +0 / 0 | 1.035× | 4 → 4 | 2 → 1 | 178.8 → 173.4 |
| topology / pcbench | 12 | 329 → 320 (-9) | 2 / 1 | +3 / 1 | +0 / 0 | 1.025× | 1 → 1 | 2 → 1 | 178.8 → 173.4 |
| topology / local | 4 | 1 → 1 (+0) | 0 / 0 | +0 / 0 | +0 / 0 | 1.122× | 3 → 3 | 0 → 0 | 50.4 → 51.3 |
| conflicts / all | 16 | 330 → 316 (-14) | 5 / 0 | +3 / 1 | +29 / 1 | 0.886× | 4 → 5 | 2 → 0 | 178.8 → 176.4 |
| conflicts / pcbench | 12 | 329 → 315 (-14) | 5 / 0 | +3 / 1 | +29 / 1 | 0.894× | 1 → 2 | 2 → 0 | 178.8 → 176.4 |
| conflicts / local | 4 | 1 → 1 (+0) | 0 / 0 | +0 / 0 | +0 / 0 | 0.816× | 3 → 3 | 0 → 0 | 50.4 → 51.0 |

## Deadline and report-cap controls

- rebuild: both sides completed normally on 14 boards: unrouted Δ +0, copper Δ +0, mask Δ +0. Mask counts below the cap on both sides: 15 boards, Δ +0, 0 gaining findings.
- topology: both sides completed normally on 14 boards: unrouted Δ +2, copper Δ +0, mask Δ +0. Mask counts below the cap on both sides: 15 boards, Δ +0, 0 gaining findings.
- conflicts: both sides completed normally on 14 boards: unrouted Δ -3, copper Δ +0, mask Δ +29. Mask counts below the cap on both sides: 15 boards, Δ +29, 1 gaining findings.

## Per-board changes

| Variant | Board | Unrouted | Copper | Mask | Either side reached deadline? |
|---|---|---:|---:|---:|---|
| rebuild | pcbench-real-time-chess_kfchess | 67 → 65 | 70 → 73 | 0 → 0 | True |
| rebuild | pcbench-karabas-nano_karabas-nano-revC | 39 → 40 | 1 → 1 | 199 → 199 | True |
| topology | pcbench-hbr-mk2_hbr-mk2-digital | 14 → 16 | 0 → 0 | 0 → 0 | False |
| topology | pcbench-real-time-chess_kfchess | 67 → 65 | 70 → 73 | 0 → 0 | True |
| topology | pcbench-karabas-nano_karabas-nano-revC | 39 → 30 | 1 → 1 | 199 → 199 | True |
| conflicts | pcbench-srambo_1_srambo_1 | 1 → 0 | 0 → 0 | 136 → 165 | False |
| conflicts | pcbench-Mouse_Mouse | 4 → 3 | 0 → 0 | 44 → 44 | False |
| conflicts | pcbench-gb-hardware_GB-CARTPP-XC | 9 → 8 | 0 → 0 | 0 → 0 | False |
| conflicts | pcbench-real-time-chess_kfchess | 67 → 65 | 70 → 73 | 0 → 0 | True |
| conflicts | pcbench-karabas-nano_karabas-nano-revC | 39 → 30 | 1 → 1 | 199 → 199 | True |
