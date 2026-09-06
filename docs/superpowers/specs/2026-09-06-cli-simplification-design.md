# CLI simplification

Date: 2026-09-06. Branch: `worktree-simplify-cli`, from `main` at `2aed1b7`.

## Goal

One command line, one settings story, one implementation shared by the CLI
and the MCP tools. Everything the binary carries only to match the classic
router's argv, messages and exit codes is removed.

## Decisions already taken

1. The legacy `-de/-do/-mp` command-line form is dropped entirely.
2. Settings reach a run from dedicated flags, `--set section.field=value`,
   and an optional `--settings <file>` JSON. The environment-variable source
   and the `FR_ROUTER_BUDGET` variable are dropped.
3. Behaviour shaped for parity is normalised: `drc` exits 1 on violations,
   logs are plain statements, reports carry the crate's own version.
4. Every in-repo consumer of the old form is updated in the same change: the
   end-to-end tests and their committed argv, the reference generator
   scripts, `quality-ab.sh`, and the benchmark's candidate runner.
5. The CLI and the MCP tools become two front ends over one set of request
   types and operations (approach C).

## Architecture

### `crates/freerouting/src/ops/`

A new module holding the request types and the four operations. `commands/*`
and `mcp/tools/*` build a request, call the operation, and render the outcome.
Neither adapter loads a board, merges settings or runs the pipeline itself.

```rust,ignore
pub enum BoardSource { Path(PathBuf), Text { text: String, name: String } }

pub struct SettingsOverrides {
    pub settings_file: Option<PathBuf>,   // --settings, priority 10
    pub set: Vec<String>,                 // "section.field=value", priority 60
    pub sparse: Option<RouterSettings>,   // the MCP payload, applied last
}

pub struct LoadRequest {
    pub source: BoardSource,
    pub rules: Option<PathBuf>,
    pub session: Option<PathBuf>,
    pub kicad_project: Option<PathBuf>,
    pub settings: SettingsOverrides,
}

pub struct RouteRequest {
    pub load: LoadRequest,
    pub output: OutputFormat,             // Ses | KicadSessionJson
    pub cancel: CancelToken,
    pub progress: SyncProgressSink,
    pub visualize: Option<RoutingVisualizationOptions>,
}

pub struct DrcRequest  { pub load: LoadRequest, pub flavor: DrcJsonFlavor }
pub struct InfoRequest { pub load: LoadRequest }
```

Dedicated flags are folded into `SettingsOverrides::set` by the CLI adapter:
`--max-passes N` becomes `router.max_passes=N`, `--timeout T` becomes
`router.job_timeout=T`. `ops` never sees a flag.

### Operations

`ops::load(&LoadRequest) -> Result<Loaded, OpError>` is the one load path:

1. Read the source (file bytes, or the text with a synthetic name) into a
   `RoutingJob`; sniff the format from the bytes. Only Specctra DSN and KiCad
   board JSON are accepted.
2. Parse the board (`fr_core::parse_board_if_needed`).
3. Resolve settings with `fr_settings::resolve_headless`, feeding
   `json_file` from `settings_file`, `dsn` from the design's own block,
   `cli_rules`/`scheduler_rules` from the rules file (explicit, else the one
   beside the design), `cli` from `set`, and `env: None`. Then apply the
   sparse payload with `apply_new_values_from`, so it outranks everything.
4. `fr_core::apply_router_settings_for_loaded_board` and
   `apply_immediate_post_load_processing`.
5. Re-read the rules file against the loaded board
   (`fr_dsn::rules_reader::read`), then the KiCad project
   (`fr_drc::apply_kicad_project`), then the session
   (`ses_reader::read` or `kicad::import_session` by extension).

`Loaded` carries the board, the transform, the metadata, the resolved
settings, the job, and the warnings. Steps 3 to 5 are what `route` does by
hand today; `drc` and `info` currently skip the design's settings block, and
`route_board` emulates a second merge of its own. All of that collapses here.

`ops::route(RouteRequest) -> Result<RouteOutcome, OpError>`: load, build a
`Ctx` from the settings (`RouterBudget::default()` with
`opt_changed_area_ms` from the settings, the deadline from
`router.job_timeout`), start the visualizer if requested, run
`RoutingPipeline::run`, serialise the session in the requested format.
`RouteOutcome` holds the session bytes and format, the final
`RoutingJobState`, the `RoutingResult`, the `RoutingJob` (for the manifest),
and the `RoutingVisualizationSummary`. It writes no files.

`ops::drc(DrcRequest) -> Result<DrcOutcome, OpError>`: load, generate the
report, compute the quality score from the loaded settings' scoring block,
serialise in the requested flavor. `DrcOutcome` holds the report, its JSON
and the violation count.

`ops::info(InfoRequest) -> Result<BoardSummary, OpError>`: load, summarise.

`OpError { Input(String), Load(String), Settings(String), Router(RouterError), Io(io::Error) }`.
The CLI adapter prints the message to stderr and exits 1. The MCP adapter
maps `Input` and `Settings` to `invalid_params`, the rest to `internal`.

### Adapters

`commands::route` builds the request from `RouteArgs` and the globals,
calls `ops::route`, writes the session to `-o`, writes the manifest when
`--result-json` is given, prints the visualization summary, and returns the
exit code. `commands::drc` writes the report to `-o` or stdout and exits 1
when the report has violations. `commands::info` prints the summary.

`mcp::tools::route_board` builds the request from its JSON arguments
(`dsn_path`|`dsn_text`, `ses_path`, `rules_path`, `output_path`,
`settings`), calls `ops::route`, and answers as today. `check_drc` and
`board_info` likewise. The `State.settings_argv` the MCP server carries today
becomes a `SettingsOverrides` built once at start-up from the globals.

## The command line

Globals: `-v`/`-vv`/`--verbose`, `--log-level <level>`, `--settings <file>`.

| subcommand | flags |
|---|---|
| `route <input> -o <output>` | `--rules <file>`, `--ses <file>`, `--kicad-project <file>`, `--max-passes <n>`, `--timeout <hh:mm:ss or seconds>`, `--result-json <file>`, `--set a.b=c` (repeatable), `--visualize <dir>`, `--visualize-every`, `--visualize-max-frames`, `--visualize-width`, `--visualize-height` |
| `drc <input> [-o <report>]` | `--rules`, `--ses`, `--kicad-project`, `--schema kicad\|freerouting` (default `kicad`) |
| `info <input>` | none |
| `mcp` | none |

The output format of `route` is the `-o` path's extension: `.ses` or
`.json`; anything else is a usage error before the board is loaded.

Removed: the legacy form; `--kicad-json` on both subcommands (the input's
format is sniffed); `--threads`, `--optimizer-improvement-threshold`,
`--update-strategy`, `--hybrid-ratio`, `--item-selection`,
`--ignore-net-classes` (dead, or reachable through `--set`); the
`--router.x=y` spelling; `FREEROUTING__ROUTER__*` environment variables;
`FR_ROUTER_BUDGET`.

Settings precedence for a run, lowest first: defaults, `--settings` file,
the design's own `(autoroute_settings)` block, the rules file, the priority-60
tier (`--max-passes`, `--timeout`, `--set`), the MCP payload. The
`fr-settings` resolver is not changed by this work.

## Behaviour

- Exit codes: 0 success, 1 failure, 2 usage (clap). `drc` exits 1 when the
  report carries at least one violation and 0 when it is clean. `route`
  exits 0 on `COMPLETED` and `TIMED_OUT` with the session written, 1
  otherwise.
- Logs go to stderr, without timestamps, worded as plain statements.
  `logging::MESSAGE_MAP` is deleted.
- The version written into DRC reports and manifests is
  `env!("CARGO_PKG_VERSION")`. `PARITY_VERSION`, `PARITY_BUILD_DATE` and
  `PARITY_JAR_REVISION` are removed from `fr-core`; `fr-drc`'s test alias
  follows the crate version.
- Routing output does not change. Every committed golden stays
  byte-identical; the only stems that may move are none, and `FR_REGOLDEN`
  is not used in this change.

## Deletions and rewrites outside `ops`

| item | action |
|---|---|
| `crates/freerouting/src/legacy.rs`, `tests/legacy_cli.rs` | deleted; `ExitCode` moves to `lib.rs` |
| `crates/freerouting/src/logging.rs` | `MESSAGE_MAP` and its test removed |
| `docs/cli-legacy-flags.md` | deleted |
| `scripts/differential/rust/src/bin/{p7t9,p8t1,p8t3,p8t7}.rs` and their `run.sh` entries and Java twins | deleted; they compare the port with the jar on legacy argv |
| `scripts/gen-cli-reference.sh` | deleted; goldens are cut with `FR_REGOLDEN=<label> cargo test -p freerouting --test cli_e2e` |
| `tests/reference/cli-*/argv.txt` (13 stems) | rewritten to the native form, keeping the `<JAVA_DIR>`/`<OUT>` placeholders |
| `tests/reference/cli-*/route.log` | deleted |
| `tests/parity/src/lib.rs` | `normalize_log`, `run_jar`, `jar_path`, `java_binary` and the log-projection types deleted; `cli_argv` unchanged |
| `crates/freerouting/tests/cli_e2e.rs` | scenario tests translated to native argv; the log rung and the legacy-only rows removed; new rows for `--max-passes`, `--timeout`, `drc` exit code |
| `crates/freerouting/tests/mcp_stdio.rs` | message-text assertions updated where wording changes |
| `scripts/quality-ab.sh`, `scripts/gen-drc-reference.sh`, `scripts/gen-batch-reference.sh` | port invocations switched to the native form; `FR_ROUTER_BUDGET=disabled` becomes `--set router.opt_changed_area_ms=0`; jar invocations unchanged |
| `benchmark/bench/candidates.py` | `Candidate.argv()` builds the legacy form for `kind = "java"` and the native form for `kind = "rust"`; `test_candidates.py` covers both |
| `crates/freerouting/README.md`, `crates/fr-core/README.md` | updated to the new surface |

## Testing

- `ops::load`: unit tests on the small in-repo boards for settings
  precedence (file below design block below rules below `set` below
  sparse), the rules re-read against the board, session import by
  extension, KiCad project application, and the three refusals (unknown
  format, unreadable path, bad `set` payload).
- `ops::route`, `ops::drc`, `ops::info`: outcome tests on the same boards;
  `drc`'s violation count on the committed `drc-dev-board` reference.
- Adapters: `cli_e2e.rs` and `mcp_stdio.rs`.
- Goldens: `the_ci_stems_match_the_jars_reference` and its slow sibling must
  pass without regoldening, which is the proof the refactor changed no
  routing.
- Benchmark: `uv run pytest` in `benchmark/`.

Each unit is written test-first.

## Out of scope

- Simplifying the two-merge precedence inside `fr_settings::resolve_headless`.
- Renaming or removing the `Java*`-named types in `fr-geometry` and
  `fr-router`.
- The MCP protocol surface, which is unchanged.
