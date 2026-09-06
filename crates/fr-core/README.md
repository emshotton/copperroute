# `fr-core`

The composition layer between `fr-router` and the `freerouting` binary:
loading and saving boards, the routing job, cancellation and progress, the
result manifest, the board summary, and `RoutingPipeline::run`, which ties
routing, the final design-rule check and the unrouted report together.

If you want to **route a board from code**, this is the crate you call.
Everything below it is the router, the design-rule checker, the readers and
the settings ladder.

```rust,ignore
let mut job = RoutingJob::default();
job.set_input(&dsn_path)?;
let loaded = fr_core::load::load_board_if_needed(&mut job)?;     // parse, resolve settings, apply the overrides
let mut board = loaded.board;

let sink = SyncProgressSink::default();
let ctx = Ctx::new(&loaded.settings, &sink);                     // CancelToken::new(), RouterBudget::default()
let result = RoutingPipeline::run(&mut board, &ctx)?;

fr_core::save::save_as_specctra_session_ses(&board, &loaded.transform, &design_name, &mut out)?;
```

## What this crate is not

**It is not a new home for anything `fr-router` owns.** `fr_core::{BoardStatistics,
RoutingEvent, ProgressSink, PassRecord, TaskState, RouterStop, RouterBudget,
run_pipeline, prepare_board, build_unrouted_report, …}` are `pub use
fr_router::…`, and `RoutingPipeline::run` **wraps** `run_pipeline`. This
crate is partly a façade: a reader looking for where a routing decision is
made will find it in `fr-router`, even though the call came through here.

## Layout

| Module | What it holds |
|---|---|
| `ctx.rs` | `Ctx { settings, cancel, progress, budget }` and `RoutingResult { stats, unrouted_report, drc_violations, timed_out, pipeline }` |
| `pipeline.rs` | `RoutingPipeline::run(&mut Board, &Ctx) -> Result<RoutingResult, Error>` |
| `cancel.rs` | `CancelToken` and `Deadline` |
| `progress.rs` | `SyncProgressSink`, a `ProgressSink` that can be read from another thread, and its `SyncProgressSinkView` |
| `job.rs`, `file_details.rs` | `RoutingJob`, `BoardFileDetails`, `FileFormat`, `RoutingJobState`, `RoutingStage`, `JobId`/`SessionId` (`Uuid128`) |
| `load.rs` | `load_from_specctra_dsn`, `load_from_kicad_json`, `parse_board_result`, `apply_router_settings_for_loaded_board`, `load_board_if_needed` |
| `save.rs` | `save_as_specctra_session_ses`, `calculate_crc32_for_board` |
| `manifest.rs` | `RoutingResultManifest`, the `--result-json` document |
| `summary.rs` | `BoardSummary` and `summarise`, what `freerouting info` and the `board_info` tool print |
| `stats_json.rs`, `stats_from_bytes.rs` | the board statistics as JSON, and the counters that are read from the file bytes rather than the board |
| `timespan.rs` | `parse_timespan_seconds` and the job-timeout ladder |

## `CancelToken` and `RouterStop`

A `CancelToken` is the cross-thread handle; it maps onto the router's
`RouterStop`, which is a **three**-state flag, and the mapping keeps the
distinction:

| `CancelToken` | `RouterStop` | what it means |
|---|---|---|
| `cancel()` | `ALL` | the router **and** the optimizer stop |
| `cancel_auto_router()` | `AUTO_ROUTER_ONLY` | the router stops between connections; the optimizer still runs |

`max_items` reaches the first and therefore *silently disables the
optimizer*; `max_passes` reaches the second and does not. A single boolean
collapsing the two would change behaviour without changing a test, which is
why the token exposes both. The help text for `--set` in
`crates/freerouting/src/cli.rs` says so, because the program has no
`--max-items` flag of its own — the surface for it is the generic override,
`--set router.max_items=N`.

**An uncancelled token is a no-op.** `CancelToken::default().as_router_stop()`
is indistinguishable from `RouterStop::new()`, and `apply_to` on it writes
nothing.

The router polls the token at four sites: the pass-loop heads in
`pipeline::{batch_loop, fanout, optimizer}`, and the item loop in
`pipeline::pass_runner`. The fourth exists because one pass of a large board
can take minutes in release, and a cancellation that is observed only
between passes is not a cancellation an operator can use.

## The job deadline

`Deadline` has two instants, not one: `stop_at`, when the token flips to
`ALL`, and `timed_out_at`, thirty seconds later, when the job is reported as
timed out. The ladder that builds it from `--timeout` / `router.job_timeout`
is: parse; clamp **from above only** at `MAX_TIMEOUT_SECONDS` (24 h);
`started_at + timeout`. There is **no lower clamp**, so a negative timeout is
a deadline in the past.

`Deadline::is_timed_out_at` and `RouterStop::is_timed_out` are two flags for
two questions. The first is the **job's** deadline; the second is a
**stage's** budget (the tightener's, the fanout per-pin clock, the
optimizer's own deadline), which rises the moment the stage observes
expiry. What the CLI reports is the job flag alone: `commands::route` reads
`Deadline`, never `PipelineResult::timed_out`, which folds the stage budgets
together. `crates/freerouting/tests/cli_e2e.rs::a_stage_timeout_is_not_a_job_timeout`
is the test that keeps them apart.

## `Ctx` has no rng seed and no `max_threads`

Spec §10 mentions both. This crate has neither: there is no randomness on any
routing path that is not derived from the settings, and `max_threads` is
read by nothing — the only threads in the workspace are the MCP transport's
two, and they carry no routing policy. `Ctx` also holds no board and no
job; the board is `RoutingPipeline::run`'s `&mut Board` parameter, owned by
whichever caller loaded it.

## `RoutingPipeline::run` composes; it does not decide

Three additions over `run_pipeline`, and **no fourth**: the `CancelToken`
adaptation, the final board's DRC violations, and the unrouted report.
`RoutingResult::stats` **is** `PipelineResult::final_statistics`, never a
second `BoardStatistics::compute` — the statistics constructor runs the
design-rule checker, and `fr-drc`'s memoisation makes the call count
observable. `RoutingResult::unrouted_report` is the diagnostic text; the
**count** comes from `stats.connections.incomplete_count`.

## The load sequence

Loading a board is `load_from_specctra_dsn` (or `load_from_kicad_json`) →
`apply_parsed_board_result` → **`apply_router_settings_for_loaded_board`** →
`apply_immediate_post_load_processing`, with `load_board_if_needed` in front
of them. The third step has four parts that split two and two:

| step | who owns it |
|---|---|
| if the board's layer count differs from the settings', write the board's into the settings | **settings** — `apply_router_settings_for_loaded_board`, and `fr_settings::resolve_headless` |
| `apply_board_specific_optimizations(board)` | **settings** — the same two |
| the copper-to-edge clearance override | **board** — `fr_router::pipeline::prepare_board` → `Board::apply_copper_to_edge_clearance_override` |
| the hole clearance override | **board** — the same, → `apply_hole_clearance_override` |

The two settings steps have two callers and one implementation each.
`resolve_headless` runs them because it models the whole settings ladder;
`load.rs` runs them because it is the loader. They are **not** nested — the
loader has no `SettingsInputs` ladder to hand `resolve_headless`. The order
is settled at the one place the two meet, the CLI: parse the board, call
`resolve_headless` **once** against the parsed board, then run the loader's
two board passes with the resolved settings.
`tests/load.rs::the_settings_pass_is_the_same_two_steps_resolve_headless_runs`
asserts the two agree, so the duplication cannot drift.

The two board steps run **once** per load, after the design is read.

## Versions

```
PARITY_VERSION      = "2.3.1-SNAPSHOT"
PARITY_BUILD_DATE   = "2026-09-01"
PARITY_JAR_REVISION = "278fe14123c49376667239659c98d41a597acce9"
SERVER_VERSION      = env!("CARGO_PKG_VERSION")
```

`PARITY_VERSION` is the version string written into the DRC report's
`freerouting_version` and the manifest's `app_version`, so documents the
binary writes stay comparable with the committed references. The
SES/DSN `(host_cad …)`/`(host_version …)` are echoed back **from the input
file** and never carry it. `SERVER_VERSION` reaches the MCP `serverInfo` and
nothing else.

## The result manifest (`manifest.rs`)

`RoutingResultManifest` is the `--result-json <path>` document: thirteen
keys in declaration order, written through the same
`to_gson_string_pretty` the settings and the statistics go through, with no
trailing newline. **The Rust field order is the JSON key order** — serde's
derived `Serialize` streams a struct in declaration order — so reordering the
struct changes the file.

Three things about it are worth knowing before reading the code.

**`board_statistics` is serialised through `GsonBoardStatistics`**, not
through `serde_json::Value`: `Value`'s map alphabetises keys and its `f32`
widens to `f64`, and both would change the bytes. `normalized_score:
572.4359` in the committed `p8t2` transcript is what proves the single-
precision path end to end.

**The SHA-256 is hand-written.** `sha256_hex` needs one and the workspace
adds no dependency for it; the FIPS 180-4 core is sixty lines in
`manifest.rs`, pinned against the three NIST example vectors, the empty
message and the 55/56/63/64/65-byte padding boundaries.

**`resolve_git_sha` reads the environment**, `FREEROUTING_GIT_SHA` first and
`freerouting.git.sha` second, and trims the value with its own whitespace
set rather than `str::trim`; `whitespace_sets_are_the_measured_ones` pins
that set.

## The job model (`job.rs`, `file_details.rs`)

`RoutingJob` carries the file details of the input, output, rules and DRC
files (`BoardFileDetails`: path, name, format, derived output name), the
job's `RouterSettings` and `DesignRulesCheckerSettings`, its state and
stage, and resource usage. `FileFormat` is detected from the file bytes and
has nine values (`Dsn`, `Frb`, `Ses`, `Rules`, `Scr`, `DrcJson`,
`KicadDesignJson`, `KicadSessionJson`, `Unknown`). `RoutingJobState::Invalid`
is a terminal state — an input that is neither a design nor a KiCad board
fails the job rather than leaving it queued.

`Session` collapses to `SessionId` (a 128-bit newtype) plus
`validate_session_host`. There is no random id: `RoutingJob::new` uses
`Uuid128::NIL` and `RoutingJob::with_id` takes one from a caller that has one
(the MCP server mints them). Nothing on a decision path reads an id.

**The path helpers in `job.rs` are POSIX-only**: they hardcode `/` as the
separator and `starts_with('/')` as the definition of an absolute path, so
every derived output name goes through the same rules on every host.
Windows support is a rewrite of that module, not an unpinning of one
constant.

## Tests

`cargo test -p fr-core` runs the unit tests plus eleven integration suites:
`cancel` (the three-state mapping, the no-op property, a real cross-thread
cancel), `ctx`, `job` (154 file-detail rows from `tests/data/p8t1-job-model.txt`,
plus 36 named edge cases), `load`, `manifest`, `overrides` (the 54 stage
blocks of `tests/data/p8t3-clearance-overrides.txt`, search-tree leaf count
and order digest included), `pipeline` (the whole pipeline with a recording
sink, comparing session bytes), `register`, `stats`, `summary` and
`timespan` (the 30 rows of `tests/data/p8t0-timespans.txt`).

The suites that read a board fixture take it from the fixture corpus in a
sibling `../freerouting` checkout (`FREEROUTING_JAVA_DIR` overrides the
location) and skip with a printed message without it.
