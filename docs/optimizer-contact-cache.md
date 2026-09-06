# Optimizer contact queries and split snapshots

Cycle checks reuse item contact sets only while `normalize_traces_checked` or
`normalize_all_traces_checked` is running. The private scope clears its scratch
on return, error, cancellation, and unwind. Queries outside those wrappers are
uncached. Cloning a board drops scratch; scratch does not affect board equality.

Any item geometry, net, layer, or search-tree mutation reachable inside that
scope must invalidate cached contacts before another query. Keep invalidation
at the mutation site, including direct item/tree edits. Public board fields
and incomplete revision tracking make a persistent revision-keyed cache unsafe.
Debug builds check cache hits against fresh contacts; release builds omit that
validation. The mutex preserves `Board: Sync` and is released before validation.

Trace splitting snapshots each distinct overlapping item once per query.
Entries remain sorted by object and shape index. Subsequent queries refresh live
items while retaining snapshots of removed items for the existing fallback.

## Benchmark

Given an input DSN and a compatible routed session:

```sh
FR_BENCH_SEARCH_STEPS=1000000 FR_BENCH_OUTPUT=/tmp/optimized.ses \
  cargo run --release -p freerouting --example optimizer_bench -- \
  INPUT.dsn INPUT.ses 20 120000
```

Arguments are DSN, session, maximum candidates, safety deadline in milliseconds,
and an optional repeat window in milliseconds for sampling short workloads.
The example runs one optimizer pass on one thread. It uses the shared board
result parser (including partial-load warnings) and immediate post-load
processing before session import. Only imported traces/vias are unlocked;
original fixed items stay fixed. `FR_BENCH_SEARCH_STEPS` sets an optional fixed
work allowance; `FR_BENCH_OUTPUT` optionally writes a session for comparison.

JSON output reports optimizer time, work counts, outcomes, and before/after
quality. Timing excludes loading, external statistics, and session writing.
For comparisons, build each version with the same harness, alternate execution
order, compare work and session bytes, and exclude deadline-limited runs from
equal-work speedup claims.

## Measured example

Five-run medians on Apple M1 Pro from the initial PR benchmark:

| Board | Before both changes | Contact cache | Both |
| --- | ---: | ---: | ---: |
| Data Manager | 4.195 s | 4.158 s | 2.001 s |
| STM32 | 1.634 s | 1.606 s | 1.149 s |
| M2FC | 8.094 s | 4.617 s | 4.189 s |

All 45 runs matched session bytes and work/quality metrics. These historical
measurements used the earlier harness without immediate post-load processing;
they are not measurements of the corrected loading path. They measure bounded
optimizer attempts, not full routing. Under a wall-clock deadline, faster code
may perform additional work and produce different output.
