# Corridor guidance enabled by default

Related signals now receive corridor guidance without an environment variable. Its existing final post-shove path check uses the same predicate. `COPPERROUTE_CORRIDOR_GUIDANCE=0` disables guidance; unset or `1` enables it. No routing-cost or insertion algorithms changed. The rejected protected-prefix prototype is excluded.

The user requested default activation after reviewing the measured trade. Individual board regressions remain counted.

## Full-corpus validation

`quality-epyc-corridor-default-full-01` compares main `f9d4be5`, its previous opt-in setting, the new default, and the explicit opt-out. All four variants attempted 751 boards (740 PCBench + 11 local); all 3,004 KiCad referees succeeded after any required retries on unchanged sessions. Ten passes, 300-second budget, one routing thread, 192 workers, same project and pad rules. No Java referee.

| Main → new default, both completed | Boards | U improved / regressed | Net unrouted | Copper gainers | Net copper | CPU ratio |
|---|---:|---:|---:|---:|---:|---:|
| pcbench | 709 | 29 / 12 | -39 | 6 | -1 | 1.0054 |
| local | 10 | 0 / 1 | +3 | 1 | +1 | 1.0285 |

| Identity control, both completed | PCBench pairs | Local pairs | Changed sessions | Net U / copper |
|---|---:|---:|---:|---:|
| optin/default | 713 | 10 | 0 | 0 / 0 |
| main/disabled | 709 | 10 | 0 | 0 / 0 |

Default activation reproduces the already-tested opt-in routes byte-for-byte on all completed pairs. The explicit opt-out likewise reproduces previous-main routes. Deadline-affected outputs remain in the raw report but are not used to infer algorithmic quality gains. CPU ratios are single-run observations, not speed claims. Mask counts can vary even with identical sessions and are not used to justify the change.

## Tests and limits

The default/opt-out environment test and the default final-path-safety replay failed before the change and passed after it. `cargo test --workspace` passed 2,608 checks including isolated child checks, zero failures, 77 ignored. No JVM recordings changed. The benchmark source/data/test manifest covers 602 files.

Known board-level regressions from corridor guidance are accepted as part of this requested default activation, not erased by aggregate gains. The native web-import path has not received equivalent full-corpus validation.

Artifacts are in `routing-quality-artifacts/corridor-default/`; full metrics, exports, sessions and referees are preserved on workbench, with reports and exports also saved on the laptop.
