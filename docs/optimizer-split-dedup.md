# Per-query split snapshot deduplication

## Change

`Board::split_overlapping_entries` now groups adjacent overlap entries by object
and clones an item once per group, instead of once per shape entry. The overlap
query returns entries ordered by object and shape index, so all entries for one
item are adjacent. The returned entries and their order are unchanged.

This replaces two production lines. It adds no persistent cache or invalidation
machinery. Each subsequent query still refreshes snapshots of live items, and
the snapshot map retains removed items for the existing fallback path.

The regression test creates two multi-segment traces, verifies multiple entries
per item and sorted results, compares snapshots to live items, removes one item,
changes the other, and verifies both snapshot refresh and removed-item retention.
It also checks an empty query. It passed before and after the optimization:
this is a behavior-preservation test, not a test that counts clone calls.

## Focused benchmark

Five pairs per board, alternating baseline/change order. Baseline already has
the retained normalization contact cache; none of the three discarded caches
is present in either build.

| Board | Baseline median | Deduplicated median | Time reduction | Speedup |
| --- | ---: | ---: | ---: | ---: |
| Data Manager | 4.189 s | 1.996 s | 52.4% | 2.10x |
| STM32F373 LQFP48 | 1.603 s | 1.147 s | 28.4% | 1.40x |
| M2FC | 4.642 s | 4.199 s | 9.5% | 1.11x |

All focused pairs produced byte-identical sessions and identical work counts,
before/after quality metrics, and outcomes. These are equal-work optimizer
attempts on imported sessions, not full autorouting or optimization to convergence.

## Method

The same 40-board inputs and benchmark harness as `optimizer-profile40.md`.
Apple M1 Pro, release build, one optimizer thread, one pass, at most 20 candidates,
existing search-work limit 1,000,000. A 120-second safety deadline replaces the
previous screen's 30 seconds in both builds. Sizif512ext still timed out in the
baseline at 120.111 seconds; that attempt is preserved in `sizif-120s-timeout`.
It is excluded from speedup claims and receives a separate 30-second-deadline
comparison in both builds. A spent work budget did not ensure prompt completion
for this case; that deserves separate investigation. Build and test activity
finished before timings began; sampling runs separately afterward.

The measured interval excludes input loading, external statistics, and session
serialization. The three targeted boards have five pairs. Initial slowdown
signals prompted seven total pairs for Wavegen and three for the HV switch.
Other boards receive one screening pair, so small changes on those boards are inconclusive.
Sessions are retained for byte comparisons. No boards were downloaded again.

Artifacts and reproducible driver:
`/Users/em/Development/freerouting/optimizer-split-dedup-20260907`.

- `run.py`: paired timings, session/work/quality assertions, and separate profiles.
- `runs/<board>/<repeat>/<variant>`: invocation, output, metrics, session.
- `profiles/<board>/0/<variant>`: raw macOS sample stacks and trial metrics.
- `analyze.py`, `summary.json`: per-board timings and inclusive profile shares.
- `trace_normalize.rs`, `source.patch`, `optimizer_bench.rs`: source snapshots.
- `board-tests.log`: full board test output.

Baseline binary (retained under `optimizer-profile40-20260906`):
`055ba02ea1db491cea4467935ddf877c159586a88cb8221c0c452431874226a9`.
Deduplicated binary (retained in this experiment's artifacts):
`b565f48ba2abfc8f7ea3a67432912e5dac5a042864a6daebe8dee1fd8d0572dc`.

## Broader results and follow-up profiles

All 40 boards produced byte-identical sessions, work counts, quality metrics,
and outcomes between builds: 120 completed unprofiled runs, plus the separately
preserved initial 120-second Sizif timeout. The 30-second Sizif comparisons are
deadline-limited and excluded from timing comparisons. No other compared board
timed out or returned an optimizer error.

The initial Wavegen screening pair was 7.7% slower, but seven-pair medians were
4.332 versus 4.376 seconds (1.0% slower). The initial HV-switch pair was 8.2%
slower; three-pair medians were 8.757 versus 8.672 seconds (1.0% faster). These
rechecks do not establish a material regression. No other board's screening pair
was more than 5% slower. Most non-hotspot boards were approximately unchanged.

Across the 39 non-timeout boards, the sum of per-board medians was 132.069 versus
128.583 seconds (2.6% less). This is a descriptive screen aggregate with unequal
repeat counts, not an estimated corpus-wide speedup. The strong claim is the
repeatable improvement on the three copying-heavy boards.

Separate five-second profiles show the intended hotspot shrinking:

| Board | Item/header clone share, before → after | Split-query helper share, before → after |
| --- | ---: | ---: |
| Data Manager | 36.78% → 1.20% | 54.14% → 4.63% |
| STM32 | 16.81% → 1.69% | 29.21% → 7.57% |
| M2FC | 7.99% → 0.85% | 14.32% → 3.95% |

Shares are inclusive samples under the optimizer frame; they overlap and are
not additive. Short trials repeat with freshly loaded inputs. The six profiles
contain 18 optimizer trials and 3,750–3,896 optimizer samples each; self-sample
totals match root counts. Percentages are not absolute costs, but together with
unprofiled timings they support the proposed copying mechanism.

## Validation and decision

Keep the two-line production change and its regression test alongside the
existing contact cache. No additional cache was introduced.

Passed: all 695 board unit/integration tests and 35 optimizer/optimizer-items
integration tests (eight tests remain ignored),
`cargo clippy -p fr-board --all-targets -- -D warnings`,
`cargo fmt --all --check`, and `git diff --check`.

Artifacts occupy approximately 24 MiB. All session files and raw profiles are
retained locally. No commit or push was performed.
