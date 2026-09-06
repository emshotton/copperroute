# Optimizer caching follow-ups — 2026-09-06

## Decision

**Final decision: retain none of these three follow-up changes.** At the user's request, ideas 1 and 2 and their tests were also removed. Only the earlier normalization contact cache remains. The measurements below are preserved as a historical experiment record; references to the "retained" or "final" build below mean the intermediate ideas-1-and-2 build, not the current worktree.

All three ideas were tried sequentially, benchmarking and sampling after each. Ideas 1 and 2 were initially retained because they remove specific duplicate traversals with little additional machinery. Their end-to-end effects were small and noisy, not another large demonstrated speedup like the earlier normalization cache.

The third cache showed no benefit in the six-board screen: all six median optimizer times were slightly slower. Rip-up connection walking accounted for at most about 0.3% of the sampled optimizer stacks before this experiment, and at most 0.2% afterward. That evidence does not justify retaining its extra cache/API. Its prototype, test, binary and profiles are preserved outside the worktree.

## Changes

1. **Route setup connectivity reuse.** Added `Board::connection_sets`, returning connected and unconnected items together from one connected-set traversal. The maze engine previously computed the connected set inside `unconnected_set`, discarded it, then immediately traversed again. `unconnected_set` now delegates to the combined implementation. Invalid IDs, absent nets and non-positive net selectors are covered by differential tests.

2. **Candidate collection → distance ordering reuse.** Inspection showed that candidate collection already skips most duplicate connected components. Instead of assuming component roots are interchangeable, cache the exact `(ItemId, net)` query result already produced during collection. Distance ordering consumes that set rather than recomputing it. Only eligible candidates' sets are retained; there are no set clones on the handoff. The cache exists only for the read-only collection/ordering call. Debug builds compare the reused set with a fresh traversal; a unit test covers matching-net reuse and distance memoization. Existing ordering, tie-order and net-filter tests also pass.

3. **Rip-up contact cache — not retained.** Added an immutable-board batch connection-walk API with shared adjacency sets, used while collecting rip-up connections. Differential tests covered independent versus batched walks, stop policies, duplicate/missing roots, removal and rollback. The experiment preserved all measured output/work but did not improve timings. The production change and its now-inapplicable test were removed after measurement; the original connection-walk implementation was restored.

No candidate-local work-budget implementation was changed.

## Incremental timing results

Positive means less optimizer time; negative means more. Each column compares against the immediately preceding build, **not** against main. These are paired three-repeat median observations, not statistically established percentages. Do not multiply them into a claimed combined speedup.

| Board | Idea 1 | Idea 2 | Idea 3 |
|---|---:|---:|---:|
| deskbot_breakout | +1.1% | +1.5% | -3.0% |
| Hardware_Playground_serial_gw_ATMEGA328P | +3.1% | +2.0% | -0.7% |
| Biscay_Blueeye_sipm-fpga | +0.2% | +0.1% | -0.3% |
| APM-RPi-Shield_APM-RPi-Shield | +0.5% | +4.7% | -4.3% |
| DAC-ADAU1966_DAC-ADAU1966 | +1.6% | +1.4% | -0.6% |
| decelerator4030_decelerator4030 | -0.8% | -0.7% | -0.4% |

For example, idea 2 changed APM's median from 4.1533 s to 3.9583 s in its paired screen. Idea 3 changed it from 4.0247 s to 4.1971 s in a separate paired screen. The changed baseline timings illustrate run-to-run variation.

## Profiles after each step

Sampled Biscay, APM and Decelerator after each change, plus the starting baseline. Percentages are inclusive and conditioned on `BatchOptimizer::run_batch_loop`; categories overlap. Inlining and short sampling windows limit precision.

| Workload | Connectivity baseline | After 1 | After 2 | After 3 |
|---|---:|---:|---:|---:|
| Biscay | 5.91% | 5.92% | 6.20% | 6.10% |
| APM | 1.15% | 1.00% | 0.79% | 0.79% |
| Decelerator | 2.39% | 2.36% | 2.29% | 2.10% |

These profiles do not reveal a large new connectivity-cache opportunity in the paths changed here. After idea 2, maze connection routing still accounts for about 92% of APM optimizer samples and 67% of Decelerator samples. Those are broad inclusive routing categories, not proof of a particular next optimization.

## Method and correctness

- Baseline is the previous **normalization-contact-cache build**, based on `7861a66`, not unmodified main.
- Six-board screen: deskbot, ATmega gateway, Biscay, APM, DAC and Decelerator. This deliberately covers small, connectivity-sensitive and maze-dominated cases from the saved sets; it is not a new random sample.
- Each idea is cumulative during experimentation: baseline → 1 → 1+2 → 1+2+3. Three paired repeats per board, alternating which binary runs first.
- Local Apple M1 Pro, release binaries, one optimizer thread, one pass, at most 20 candidates, existing search-work limit 1,000,000; safety deadline 120 seconds. No measured run reached the deadline.
- Timed only the optimizer interval, excluding input loading, before/after statistics and session writing. No builds or sampling ran concurrently with the timed comparisons.
- Separate macOS `sample` profiles: five seconds at 1 ms. The harness repeats fresh-input runs for a six-second launch window. Long single attempts can outlast the sample window, so their profiles cover only part of the attempt.
- Final retained build (ideas 1+2) compared with the starting baseline on **all 20 saved boards**, one paired run each. This final pass checks output/work preservation; it is not a precise timing estimate.
- Across **148 timed runs and 24 profiled runs**, per-board session SHA-256, quality metrics, candidates attempted, route work, search steps and result status all matched, including across experiment stages. `verify.py` checks every recorded profile trial too.
- Fixed work limits often stop hard-board runs in their first candidate, which is rolled back. These measurements are not full routing or optimization-to-convergence results. They do not establish a quality improvement or a corpus-wide speedup.

## Verification

All final checks passed: 694 release board tests, 173 selected router tests, 112 CLI/package tests, and 379 debug board/normalization tests. Formatting and Clippy passed with the existing baseline allowances. Commands and logs are recorded in the artifact directory:

```sh
cargo test --release -p fr-board
cargo test --release -p fr-router --lib --test airline --test batch_autorouter \
  --test optimizer --test optimizer_items --test pipeline --test reference_parity
cargo test --release -p freerouting
cargo test -p fr-board --lib --test trace_normalize
cargo fmt --all --check
cargo clippy -p fr-board -p fr-router -p freerouting --all-targets -- \
  -D warnings -A clippy::collapsible-if -A clippy::large-enum-variant
```

The two Clippy allowances are the same pre-existing baseline allowances used in the prior experiment. No routing golden was changed. The full slow batch lane and full 605-board corpus were not rerun.

## Artifacts

Local directory: `/Users/em/Development/freerouting/optimizer-cache-followups-20260906`.

- `results/step1`, `results/step2`, `results/step3`: paired timing JSON, sessions, stderr and profiles.
- `results/final`: retained-build versus baseline output/work comparisons on all 20 boards.
- `summary.json`, `summary.txt`, `verification.json`: aggregate results and cross-stage assertions.
- `run.py`, `analyze.py`, `verify.py`: benchmark, profile analysis and verification scripts.
- `baseline`, `step1`, `step2`, `step3`, `final`: preserved measured executables; SHA-256 hashes in `verification.json`. `final` is the measured `step2` binary.
- `step2-*.rs` and `step3-*.rs`: source snapshots preserving the discarded prototype and its test.
- `retained.patch`, `retained-*.rs`: retained source snapshots; the patch also includes the earlier uncommitted normalization cache integration.
- `*-tests.log`, `clippy.log`, `final-build.log`: verification/build logs.

No additional board data was downloaded. Changes remain in the `explore-optimizer-speed` worktree; no commit, push or PR was made.
