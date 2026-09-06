# Forty-board optimizer profiling

## Result

The strongest broad opportunity is spatial-query and room-completion work. The
best small, localized experiment is reducing whole-item snapshot copies during
trace splitting. These are measured hotspots, not measured speedups: no new
optimization was implemented for this experiment.

The original normalization-scoped contact cache is enabled. All three rejected
follow-up caches remain removed.

## Dataset and reproducibility

Artifacts: `/Users/em/Development/freerouting/optimizer-profile40-20260906`.

- Source corpus on `em@workbench`:
  `/home/em/via-and-trace-length/benchmark/results/via-and-trace-length-full-r5`.
  This is a completed 605-board run; full-r6 was incomplete when inspected.
- Hard 20: prioritize historical wall time >=295 seconds, then descending
  reported unrouted count, then descending CPU time, with board-name tie breaks.
  This is a reproducible historical difficulty proxy, not a fresh ranking under
  the current implementation.
- Random 20: uniform sample without replacement from the other 585 boards,
  seed `20260907`, with no runtime or quality filtering. These groups are disjoint.
- Only selected DSNs, routed sessions, and small metadata were downloaded.
  Total compressed transfer was 1,799,286 bytes, including one corrected download.
  Two case-only PocketBone names required numbered local directories on macOS;
  the initial inputs are retained in `superseded-inputs` and were not profiled.
- STM32 had no corpus session. Its DSN was verified byte-identical to the earlier
  local benchmark input, and that benchmark's partial session was reused.
  Its exact origin is recorded in `machine.json`; it is not silently replaced by
  another board.
- Machine: Apple M1 Pro. Worktree: `freerouting-rs-optimizer-speed`, branch
  `explore-optimizer-speed`, base commit
  `7861a663b63b69a8044f79842c799325adda3826`, plus retained contact-cache changes.
- Measured executable SHA-256:
  `055ba02ea1db491cea4467935ddf877c159586a88cb8221c0c452431874226a9`.
- `selection.json` has all 40 board names and historical selection metrics;
  `machine.json` records input hashes. `summary.json` contains every board's
  timings, before/after metrics, work counts, and parsed stacks. Raw stacks are
  `profiles/<hard-NN|random-NN>/0/sample.txt`.
- `pull.py`, `prepare.py`, `run.py`, and `analyze.py` preserve the experiment.
  Per-process `argv.json` preserves the invocation; the executable is retained.

Artifacts occupy approximately 81 MiB, including the executable and raw profiles.
No per-trial session files or rendering frames were generated.

## Method and limitations

This profiles the **optimizer**, including its internal re-router, on imported
partial/final sessions. It does not measure a full autoroute or optimization to
convergence. The harness unlocks only imported session traces/vias, preserving
the input board's fixed items.

Release build; one optimizer thread; one pass; at most 20 candidates; existing
benchmark search-work cap of 1,000,000; 30-second deadline; 90-second external
watchdog. Timings cover `BatchOptimizer::run_batch_loop`, excluding input loading,
before/after statistics, and output writing. Sub-second cases have three
unprofiled process runs, reported by median; others have one. These timings are
screening baselines, not statistically strong A/B results.

Each board has a separate five-second macOS `sample` profile at a requested
one-millisecond interval. Short cases repeat fresh-load optimizer trials within
a six-second window. Analysis counts only stacks beneath the optimizer frame,
excluding repeated loading and external statistics. Long cases are sampled only
over part of their attempt. Percentages are inclusive, overlap, and must not be
added or treated as predicted whole-job speedups. Release inlining also affects
which symbol receives self samples.

All 70 unprofiled processes and 40 profile processes exited zero; every profiler
succeeded; no optimizer errors or watchdog kills occurred. Profiles contain
3,543–4,040 optimizer samples each, with self-sample totals matching the root.
Sizif512ext hit the 30-second deadline cleanly in both its baseline and profile
trial. All hard cases reached the work cap while attempting their first candidate,
and their recorded quality metrics were unchanged. Thus these hard-case profiles
emphasize difficult failed/rolled-back attempts, not later optimizer behavior.
Three random cases also reached the work cap: Antdroid, AVR fuser, and Mailbox.

## Measured hotspots

Percentages below are per-board medians of inclusive optimizer-stack shares,
not time-weighted corpus totals.

| Category | Hard 20 | Random 20 |
| --- | ---: | ---: |
| Spatial overlap queries | 30.68% | 36.66% |
| Room completion | 34.83% | 16.90% |
| Recursive cycle checks | 1.92% | 1.87% |
| Connected-set traversal | 1.52% | 4.96% |
| Item/header cloning | 0.92% | 1.86% |
| Board deep-copy snapshots | 0.02% | 0.95% |

Bounded optimizer times range from 1.584–30.111 seconds for hard boards and
0.007–7.536 seconds for random boards. The random sample supports spatial-query
work as a broader target than another connectivity cache. A high percentage on
a millisecond-scale board alone is not a strong priority signal.

### 1. Deduplicate trace-splitting snapshots within each query

| Board | Baseline seconds | Split query helper | Item/header clone |
| --- | ---: | ---: | ---: |
| Data Manager (`hard-15`) | 4.295 | 53.52% | 35.30% |
| STM32 (`hard-09`) | 1.584 | 32.07% | 17.76% |
| M2FC (`hard-18`) | 4.778 | 14.73% | 7.85% |

In `crates/fr-board/src/board/trace_normalize.rs:468`,
`split_overlapping_entries` clones an entire item for every overlapping tree
entry and inserts it into a map keyed by item ID. Multiple shape entries for the
same item can repeatedly clone, replace, and drop the same snapshot. Item headers
include cached tree entries, so these are not necessarily small copies.

Data Manager's leading self-attributed symbols are vector copying (22.78%),
`memmove` (13.79%), and dropping `ItemHeader` (11.96%). Raw stacks connect these
operations to the helper's `item.clone()` and map replacement. The evidence
establishes expensive copying, but does not yet quantify the duplicate-entry
rate or how much copying a deduplication change would eliminate.

First experiment: snapshot each distinct item once per query result, preserving
the existing entry iteration order. Do not use a permanent `entry.or_insert`:
the snapshot map survives subsequent queries after splitting and must refresh
changed items. Removed-item fallback snapshots must remain available. Add tests
for repeated shape entries, refresh after mutation, and removed-item fallback.

This is a localized redundant-work reduction, not a new persistent cache.

### 2. Reduce spatial-query allocation and geometry overhead

`crates/fr-board/src/datastructures/shape_tree.rs:484` collects overlap hits into
a fresh `BTreeSet`. The clearance path in
`crates/fr-board/src/searchtree/shape_search_tree.rs:671` then constructs another
`BTreeSet`, ordered by clearance, before returning a vector.

Prototype contiguous collection plus sorting/deduplication. Preserve exact
`TreeEntry` ordering, clearance ordering, and entry-counter tie semantics: these
can affect routing choices. Compare allocation counts as well as wall time.
The current tree already stores nodes in a contiguous vector arena; replacing
a hypothetical pointer-based tree is not an applicable recommendation.

Overlap checks repeatedly pass regular bounds through generic `TileShape`
intersection. Profile-guided specialization may help, but octagon fast paths
already exist. Measure actual shape-pair frequencies and fallback allocation
before extending them. On Sizif512ext, `Simplex::remove_redundant_lines` alone
receives 10.45% of self samples; its vector allocations and geometric trimming
deserve a focused allocation profile.

### 3. Improve room-completion traversal

Room completion accounts for 48–50% on the three PocketBone boards and 55.82%
on Wavegen. In `crates/fr-router/src/autoroute/tree_ext.rs:548`,
`complete_shape_45` walks tree nodes, tests bounds, and clips candidate rooms
against obstacles. Instrument node visits, rejected bounds, layer/net rejections,
and clipping counts to distinguish excessive traversal from expensive geometry.

Potential experiments include avoiding unnecessary whole-node copies and
specializing bounds tests. Push filtering earlier only if rejection counts
justify it. The implementation already reuses room-result buffers and incrementally
maintains bounds; those are not missing optimizations. Preserve node-generation
validation and geometric boundary semantics.

## Quality observation requiring separate investigation

Antdroid (`random-05`) changes reported unrouted count from 19 to 17 but reported
clearance violations from 0 to 9; length increases from 14637.97 to 15471.33.
This is an observation in the current cache-enabled baseline, not evidence that
the cache caused it. Reproduce against the cache-disabled build and inspect the
violations before attributing a cause. Future speed experiments must compare
quality and output, not just elapsed time.

## Recommended next experiment

Start with per-query item snapshot deduplication: the implementation surface is
small and the copy-heavy boards provide a clear diagnostic target. Then test
spatial-query collection changes for broader coverage, followed by instrumented
room-completion/geometry work. Keep each change independent and A/B it against
this retained-cache baseline, using repeated alternating runs, identical inputs
and work budgets, output comparisons, and targeted correctness tests. Extend
promising results to longer runs because these profiles intentionally bound work.
