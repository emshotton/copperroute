# Combined optimizer improvements

Fresh three-way comparison: original production baseline (`7861a66`),
normalization contact cache alone, and that cache plus per-query split snapshot
deduplication. Five runs per version per board, rotating version order.

| Board | Original | Contact cache | Both | Combined speedup |
| --- | ---: | ---: | ---: | ---: |
| Data Manager | 4.195 s | 4.158 s | 2.001 s | 2.10x |
| STM32 | 1.634 s | 1.606 s | 1.149 s | 1.42x |
| M2FC | 8.094 s | 4.617 s | 4.189 s | 1.93x |

All 45 runs agree per board on session bytes, quality metrics, work counts,
and outcomes. These are median times for bounded, equal-work optimizer attempts,
not full autorouting or convergence. Faster code can do more work under a
wall-clock deadline, so deadline-limited production output need not be identical.

Apple M1 Pro, release build, one thread, one pass, at most 20 candidates,
1,000,000 search-work limit, 120-second safety deadline. None timed out. The
benchmark harness is byte-identical between the saved baseline and current
version. Input loading, external statistics, and session writing are outside
the measured interval.

Local reproducibility artifacts:
`/Users/em/Development/freerouting/optimizer-stacked-20260907` contains `run.py`,
`results.json`, `summary.json`, binary paths and hashes in `binaries.json`, and
every run's invocation, stdout, stderr, and session. Inputs are the three named
boards from the [40-board profile](optimizer-profile40.md).

See [contact-cache results](optimizer-contact-cache.md) and
[snapshot-deduplication results](optimizer-split-dedup.md) for the broader
comparisons, implementation details, tests, and limitations. The three
[rejected follow-up caches](optimizer-cache-followups.md) are not included.
