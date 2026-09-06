# Optimizer contact cache — 2026-09-06

## Outcome

Implemented a private adjacency cache for cycle queries during normalization. On the saved ten-random and ten-hard PCBench workloads, the largest measured optimizer speedups were **3.55× Biscay**, **2.86× DAC**, and **2.72× ATmega**. CoreOne improved **1.70×** and Decelerator **1.61×**. Maze-dominated and tiny workloads benefited little.

All **204 measured runs** agreed per board on session SHA-256, quality metrics, candidates attempted, route work, search steps, and completion/error status. No output golden was changed. Summing each board's median optimizer time gives **105.32 s baseline versus 74.12 s cached**, a **29.6% reduction** for this workload mix; this is not a corpus-wide or full-routing speedup estimate.

## Implementation and safety

- Cache only `normal_contacts(ItemId)` used by recursive cycle traversal, preserving descending contact order. Root selection and conduction-area cycle policy are unchanged. The public `normal_contacts` query remains fresh.
- Activate scratch only inside `normalize_traces_checked` and `normalize_all_traces_checked`. A private scope guard reuses nested scopes and discards entries on success, error, cancellation, or unwinding.
- Invalidate all entries on insertion, removal, mutable item access, movement, geometry replacement, trace changes/combines, and snapshot restoration. The mutation audit covers the normalizer's split/combine paths, including splitting another trace while returning the original requested trace.
- Do not rely on `Board::revision`: existing geometry and rollback operations do not consistently advance it.
- Public board fields prevent safely retaining this cache outside the exclusive normalization scope. Future topology-editing paths within that scope must invalidate before the next query.
- Snapshot/clone copies start without cache data; scratch does not affect board equality. Arc-backed contact sets avoid copying sets on hits. A mutex preserves `Board: Send + Sync`, verified by a compile-time test.
- Debug builds compare each cached hit with a fresh query. Tests also compare cycle answers against a cache-free board clone.
- No candidate-local budget implementation changes, persistent board-wide cache, or public cache-statistics API. Actual optimizer timings and differential outputs were used to validate useful reuse; live production hit/miss counts are not recorded by this version.

## Benchmark method

Baseline production revision: `7861a66`. Local Apple M1 Pro; release builds, one optimizer thread. Both binaries use the same example harness and frozen DSN/SES inputs from the earlier hard/random selections. Newly imported routing is unlocked; original fixed items remain fixed.

One optimizer pass, at most 20 candidates, existing search-work limit set to **1,000,000**, and a 120-second safety deadline. No run reached that deadline. Fixed work limits prevent faster code simply doing more work before a timeout. Most hard cases exhaust the work allowance on their first candidate and roll it back: these measure equal-work optimization attempts, not optimization to convergence or better routing quality.

Three paired repeats per board, alternating execution order. Initial compilation overlapped some of the first three tiny workloads; all six sub-second random workloads were subsequently rerun in seven clean paired repeats after all builds/tests finished. The table uses those clean medians for the six small workloads and three-pair medians for the others. No profiling sampler ran during the timings. Differences of a few percent, especially below a second, should not be interpreted as established wins or regressions.

| Board | Baseline seconds | Cached seconds | Speedup |
|---|---:|---:|---:|
| kicad-guitar-preamp_Preamp-Instructables | 0.0302 | 0.0295 | 1.03× |
| deskbot_breakout | 0.8055 | 0.8085 | 1.00× |
| mechkeys_lfk78-jtag | 0.0269 | 0.0267 | 1.01× |
| Hardware_Playground_serial_gw_ATMEGA328P | 6.0784 | 2.2349 | 2.72× |
| Biscay_Blueeye_sipm-fpga | 7.4591 | 2.0992 | 3.55× |
| raspberry_pi_pullup_button_pullup_shutdown_button(revB) | 0.0138 | 0.0140 | 0.99× |
| arduino_arduino leds | 0.2052 | 0.1697 | 1.21× |
| APM-RPi-Shield_APM-RPi-Shield | 4.2365 | 4.0742 | 1.04× |
| DAC-ADAU1966_DAC-ADAU1966 | 6.8333 | 2.3914 | 2.86× |
| Retro1DecodingModules_AddressDecoderModule | 0.1043 | 0.1040 | 1.00× |
| decelerator4030_decelerator4030 | 14.5861 | 9.0385 | 1.61× |
| karabas-nano_karabas-nano-revG | 8.1338 | 7.3742 | 1.10× |
| zx-sizif-xxs_sizif-xxs | 5.3697 | 4.7773 | 1.12× |
| karabas-nano_karabas-nano-revC | 7.8258 | 7.0432 | 1.11× |
| karabas-nano_karabas-nano-revB | 8.3351 | 6.7685 | 1.23× |
| karabas-nano_karabas-nano-revA | 7.0282 | 6.7002 | 1.05× |
| CoreOne-xCORE200-Original_CoreOne | 16.1465 | 9.5119 | 1.70× |
| bms-8s50-ic_bms-8s50-ic | 4.7211 | 4.5951 | 1.03× |
| STM32F373_LQFP48_STM32_LQFP48 | 1.6273 | 1.6180 | 1.01× |
| pocketbone-kicad_pocketbone-kicad | 5.7553 | 4.7446 | 1.21× |

## Correctness and tests

TDD: insertion/removal tests first failed with stale cached contact sets; both passed after mutation invalidation was wired in.

Eight targeted tests cover contact reuse, insertion/removal, geometry replacement, movement, net changes, rollback, splitting other traces, combining, positive/broken cycles, conduction-area policy, cross-layer via cycles, layer changes, nested scopes, unwind cleanup, and normalization stop boundaries.

Passed:

- `cargo test --release -p fr-board`: 693 tests.
- `cargo test -p fr-board --lib --test trace_normalize`: 378 tests, with debug cache-hit validation enabled.
- `cargo test --release -p fr-router --test optimizer --test optimizer_items --test pipeline --test reference_parity`: 67 tests.
- `cargo test --release -p freerouting`: 112 tests, including CLI end-to-end checks.
- `cargo fmt --all --check` and `git diff --check`.
- Clippy for board/router/CLI, all targets, with warnings denied except the previously documented baseline `collapsible_if` and `large_enum_variant` lints.

This does not establish correctness on every corpus board or measure unbounded optimization. The full slow batch fixture lane and the full 605-board corpus were not rerun.

## Reproduction and artifacts

Harness: `crates/freerouting/examples/optimizer_bench.rs`. For one fixed-work run:

```sh
FR_BENCH_SEARCH_STEPS=1000000 FR_BENCH_OUTPUT=/tmp/cache-bench.ses \
  cargo run --release -p freerouting --example optimizer_bench -- \
  INPUT.dsn INPUT.ses 20 120000
```

The two environment variables are benchmark-example controls, not new production CLI options. The timed interval excludes loading, before/after statistics, and session serialization.

Local evidence directory: `/Users/em/Development/freerouting/optimizer-cache-ab-20260906`.

- `results.json`: initial 120 runs; `fast-results.json`: 84 clean small-workload reruns.
- `run.py`, `summarize.py`: paired benchmark driver and assertions over all output/work/quality comparisons.
- `random/`, `hard/`, `fast/`: per-run JSON, stderr, and output sessions.
- `baseline`, `cached`: preserved measured executables.
- `contact_cache.rs`, `integration.patch`, `optimizer_bench.rs`: implementation/harness snapshots.
- `*-test.log`, `clippy.log`: verification logs.
- Input selections remain in the sibling `optimizer-random10-20260906` and `optimizer-benchmark-20260906` directories. No additional board data was downloaded.

Measured executable SHA-256:

- Baseline: `293f24a701422bdf2497aa4510779a15980687b3af113b92eb51da8c41ac7b69`
- Cached: `0333725aa2918532a092b785b075484c7d1e2a7ab7cf0e0b90dffe1139c6ecf3`
