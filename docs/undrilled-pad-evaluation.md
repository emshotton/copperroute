# Native undrilled-pad correction

The native JSON reader leaves drill metadata unset for surface-mount pads whose drill diameter is zero. The padstack fallback then estimates a hole from the copper pad size. Applying a project hole-clearance rule can enlarge the pad obstacle around that fictitious hole and block nearby routing.

The correction stores explicit zero drill metadata for single-layer undrilled pads. Real drilled pads retain their existing handling; undrilled multi-layer legacy padstacks retain their previous fallback. No routing algorithm or default setting changes.

The regression test creates a 0.8 × 0.25 mm rectangular surface-mount pad with drill zero. On main it fails with an invented 56.25-unit drill radius. With the correction it passes, and applying the hole-clearance override leaves the pad's tree shape unchanged. No JVM recordings were changed.

The earlier `pass-repair-native-02` comparison isolates this reader correction with the structural algorithms disabled: Nano improves from two unrouted to zero, with four identical KiCad hole-clearance errors on both sides. The same four errors are present on the original human-routed Nano board, with matching type, description and pad UUIDs. Feather remains seven unrouted and 35 routing errors. That control binary also contains disabled experimental scaffolding; this branch removes the scaffolding and receives its own validation before publication.

The isolated branch is based on main f91d8496. Its source snapshot was checked against 656 source/build/test file hashes on the server; the only later code-file change is formatting the new regression test. Full workspace verification passed 2618 harness checks with zero failures and 77 ignored checks. The final workspace rerun after formatting also passed (terminal exit 0, 2618 checks, zero failures, 77 ignored).

The isolated native evaluation reproduces Nano 2→0 unrouted with the same four inherited hole-clearance findings. The initial Feather pair reached the 300-second deadline under benchmark load; a separate 600-second control completed both sides, producing byte-identical board JSON, seven KiCad unrouted and 35 routing findings. CPU was 322.28 vs 319.85 seconds; no timing improvement is claimed. The two native reports are preserved in [native-01.json](routing-quality-artifacts/undrilled-pads/native-01.json) and [native-02.json](routing-quality-artifacts/undrilled-pads/native-02.json).

## Full KiCad corpus

`undrilled-full-01` completed all 751 boards with valid KiCad referee results and no failed/unjudged cells. It used 112 workers, one router thread, ten passes and a 300-second deadline. The baseline is fresh main f91d8496, `pass-repair-main-01`. This is a DSN compatibility check; the native evaluations directly exercise the changed JSON reader.

| Candidate | Unrouted | Copper findings | Mask findings | Fully connected | Unfinished | Deadline | CPU seconds | Max cell RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| main | 4853 | 858 | 13747 | 601 | 150 | 46 | 45022.49 | 293.2 |
| undrilled | 4844 | 862 | 13746 | 601 | 150 | 42 | 45039.41 | 292.4 |

Copper is the benchmark routing-violation total; mask findings are reported separately and some are capped by KiCad at 199. Fully connected does not mean DRC-clean. Deadline includes the internal router timeout, even when the process exits successfully.

| Group | Subset | Boards | Improved | Regressed | Δ unrouted | Copper gainers | Δ copper | Mask gainers | Δ mask | CPU ratio |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| all | all | 751 | 6 | 5 | -9 | 1 | +4 | 1 | -1 | 1.0004 |
| all | both completed | 705 | 0 | 0 | +0 | 0 | +0 | 0 | +0 | 0.9990 |
| pcbench | all | 740 | 6 | 5 | -9 | 1 | +4 | 1 | -1 | 1.0003 |
| pcbench | both completed | 696 | 0 | 0 | +0 | 0 | +0 | 0 | +0 | 0.9990 |
| local-kicad | all | 11 | 0 | 0 | +0 | 0 | +0 | 0 | +0 | 1.0016 |
| local-kicad | both completed | 9 | 0 | 0 | +0 | 0 | +0 | 0 | +0 | 0.9978 |

All 705 pairs that completed without deadlines have byte-identical SES files and identical unrouted/copper/mask totals. Every observed unrouted or DRC change involves deadlines on both sides. The aggregate -9 unrouted and +4 copper therefore are not claimed as effects of this native-only correction. The automated benchmark gate nevertheless reports seven quality losses: five for unrouted and two for score. Its unmodified [generated report](routing-quality-artifacts/undrilled-pads/undrilled-full-01-pr-summary.md) is retained. A separate one-pass, 1200-second paired control with optimization explicitly disabled completed all seven flagged boards without deadlines. Each pair has byte-identical SES output and identical KiCad unrouted, copper and violation-type counts. This establishes first-pass equivalence on those boards; it does not prove every later pass or optimizer step identical, and it does not replace the full evaluation. [Seven-board control results](routing-quality-artifacts/undrilled-pads/undrilled-deadline-control-03-summary.json).


[Machine-readable full results and SES identity audit](routing-quality-artifacts/undrilled-pads/undrilled-full-01-summary.json) include origin splits and per-board differences. Raw boards, logs and source provenance are preserved on workbench under `copperroute-epyc-results/web-structural-routing/server/`.

