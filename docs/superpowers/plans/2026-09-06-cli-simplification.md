# CLI Simplification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** One native command line, one settings story, and one `ops` implementation that the CLI subcommands and the MCP tools both call; the legacy argv form, the env-var settings sources and the parity-shaped exit codes, messages and version stamps are removed.

**Architecture:** A new `crates/freerouting/src/ops/` module owns the request types (`LoadRequest`, `RouteRequest`, `DrcRequest`, `InfoRequest`) and the four operations. `commands/*` and `mcp/tools/*` become adapters: build a request, call the operation, render the outcome as exit code plus files or as a JSON result. `fr-settings` and `fr-router` are untouched except for one `pub` and one normaliser field in `tests/parity`.

**Tech Stack:** Rust 2024 workspace (`cargo`, `clap` derive, `serde_json`, `thiserror`, `tracing`), Python 3 with `uv` for `benchmark/`, bash for `scripts/`.

**Spec:** `docs/superpowers/specs/2026-09-06-cli-simplification-design.md`

## Global Constraints

- `#![forbid(unsafe_code)]` in every crate root; no new workspace dependencies.
- Every committed golden under `tests/reference/` stays byte-identical. `FR_REGOLDEN` is **not** used in this change. If a golden moves, the task that moved it is wrong, not the golden.
- Comments only for unexpected behaviour (repo `CLAUDE.md`): no comments that restate the code, describe past or future state, or cite plans or tickets.
- Tests that read the fixture corpus call `parity::require_java_dir()` first and return when it is absent.
- Commit after every task with the trailer the session prescribes:
  `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>` and
  `Claude-Session: https://claude.ai/code/session_018geYSyakvFMTVKjJY2e2Kt`.
- Run commands from the worktree root `/Users/em/Development/freerouting/freerouting-rs/.claude/worktrees/simplify-cli`.

---

## File structure

| Path | Responsibility |
|---|---|
| `crates/freerouting/src/ops/mod.rs` | re-exports; `OpError` |
| `crates/freerouting/src/ops/settings.rs` | `SettingsOverrides`, `parse_set`, `resolve` (the settings ladder, one function) |
| `crates/freerouting/src/ops/load.rs` | `BoardSource`, `LoadRequest`, `Loaded`, `load` (read, parse, settings, overrides, rules, project, session) |
| `crates/freerouting/src/ops/route.rs` | `OutputTarget`, `OutputFormat`, `RouteRequest`, `RouteOutcome`, `route` |
| `crates/freerouting/src/ops/drc.rs` | `DrcRequest`, `DrcOutcome`, `drc`, `report_date` |
| `crates/freerouting/src/ops/info.rs` | `InfoRequest`, `info` |
| `crates/freerouting/src/cli.rs` | the clap surface, native form only |
| `crates/freerouting/src/lib.rs` | `run`, `ExitCode` |
| `crates/freerouting/src/commands/{route,drc,info}.rs` | CLI adapters |
| `crates/freerouting/src/mcp/tools/{route_board,check_drc,board_info}.rs` | MCP adapters |
| `crates/freerouting/src/mcp/server.rs` | `State { overrides }` |
| `crates/freerouting/src/logging.rs` | level parsing and init only |
| `crates/freerouting/tests/cli_e2e.rs`, `tests/mcp_stdio.rs` | adapter tests |
| `tests/parity/src/lib.rs` | reference helpers, minus the jar runner and log projection |
| `tests/reference/cli-*/argv.txt` | native argv per stem |
| `benchmark/bench/candidates.py`, `benchmark/tests/test_candidates.py` | per-kind argv |
| `scripts/quality-ab.sh`, `scripts/gen-drc-reference.sh`, `scripts/gen-batch-reference.sh` | native port invocations |

Deleted: `crates/freerouting/src/legacy.rs`, `crates/freerouting/tests/legacy_cli.rs`, `docs/cli-legacy-flags.md`, `scripts/gen-cli-reference.sh`, `scripts/differential/rust/src/bin/{p8t1,p8t2,p8t3,p8t5,p8t7}.rs`, `scripts/differential/java/{P8T2,P8T3,P8T5}.java`, `tests/reference/cli-*/route.log`.

---

### Task 1: `ops::settings` — overrides and the one ladder

**Files:**
- Create: `crates/freerouting/src/ops/mod.rs`, `crates/freerouting/src/ops/settings.rs`
- Modify: `crates/freerouting/src/lib.rs` (add `pub mod ops;`)
- Test: unit tests inside `ops/settings.rs`

**Interfaces:**
- Produces: `ops::OpError`; `ops::settings::SettingsOverrides { settings_file, set, sparse }`; `ops::settings::parse_set(&str) -> Result<(String, String), OpError>`; `ops::settings::resolve(&SettingsOverrides, dsn: Option<&RouterSettings>, explicit_rules: Option<&[u8]>, scheduler_rules: Option<&[u8]>, board: Option<&Board>, host: &HostEnvironment) -> Result<RouterSettings, OpError>`.

- [ ] **Step 1: Write the failing tests**

Create `crates/freerouting/src/ops/settings.rs` with only the tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use fr_settings::HostEnvironment;

    #[test]
    fn parse_set_splits_at_the_first_equals_and_requires_the_router_prefix() {
        assert_eq!(
            parse_set("router.scoring.via_costs=77").unwrap(),
            ("router.scoring.via_costs".to_string(), "77".to_string())
        );
        assert_eq!(
            parse_set("router.optimizer.hybrid_ratio=1:2=x").unwrap(),
            ("router.optimizer.hybrid_ratio".to_string(), "1:2=x".to_string())
        );
        assert!(parse_set("via_costs=77").is_err(), "no router. prefix");
        assert!(parse_set("router.scoring.via_costs").is_err(), "no =");
    }

    #[test]
    fn set_reaches_priority_sixty_and_an_unknown_field_is_refused() {
        let host = HostEnvironment::with_processors(4);
        let overrides = SettingsOverrides {
            settings_file: None,
            set: vec!["router.scoring.via_costs=77".to_string()],
            sparse: None,
        };
        let settings = resolve(&overrides, None, None, None, None, &host).unwrap();
        assert_eq!(settings.scoring.as_ref().unwrap().via_costs, Some(77));

        let bad = SettingsOverrides {
            settings_file: None,
            set: vec!["router.scoring.no_such_field=1".to_string()],
            sparse: None,
        };
        let error = resolve(&bad, None, None, None, None, &host).unwrap_err();
        assert!(matches!(error, OpError::Settings(_)), "{error}");
    }

    #[test]
    fn the_sparse_payload_outranks_set() {
        let host = HostEnvironment::with_processors(4);
        let mut sparse = fr_settings::RouterSettings::new();
        fr_settings::set_field_value(&mut sparse, "scoring.via_costs", "5").unwrap();
        let overrides = SettingsOverrides {
            settings_file: None,
            set: vec!["router.scoring.via_costs=77".to_string()],
            sparse: Some(sparse),
        };
        let settings = resolve(&overrides, None, None, None, None, &host).unwrap();
        assert_eq!(settings.scoring.as_ref().unwrap().via_costs, Some(5));
    }

    #[test]
    fn a_settings_file_sits_below_set() {
        let dir = std::env::temp_dir().join("fr-ops-settings");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("s.json");
        std::fs::write(&file, r#"{"router": {"scoring": {"via_costs": 33, "plane_via_costs": 9}}}"#)
            .unwrap();
        let host = HostEnvironment::with_processors(4);
        let overrides = SettingsOverrides {
            settings_file: Some(file),
            set: vec!["router.scoring.via_costs=77".to_string()],
            sparse: None,
        };
        let settings = resolve(&overrides, None, None, None, None, &host).unwrap();
        let scoring = settings.scoring.as_ref().unwrap();
        assert_eq!(scoring.via_costs, Some(77));
        assert_eq!(scoring.plane_via_costs, Some(9));
    }

    #[test]
    fn a_missing_settings_file_is_refused() {
        let host = HostEnvironment::with_processors(4);
        let overrides = SettingsOverrides {
            settings_file: Some(std::path::PathBuf::from("/nonexistent/s.json")),
            set: Vec::new(),
            sparse: None,
        };
        assert!(matches!(
            resolve(&overrides, None, None, None, None, &host),
            Err(OpError::Settings(_))
        ));
    }
}
```

Create `crates/freerouting/src/ops/mod.rs`:

```rust
pub mod settings;

pub use settings::SettingsOverrides;

#[derive(Debug, thiserror::Error)]
pub enum OpError {
    #[error("{0}")]
    Input(String),
    #[error("{0}")]
    Load(String),
    #[error("{0}")]
    Settings(String),
    #[error(transparent)]
    Router(#[from] fr_router::RouterError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl From<fr_core::Error> for OpError {
    fn from(error: fr_core::Error) -> Self {
        match error {
            fr_core::Error::Router(inner) => OpError::Router(inner),
            fr_core::Error::Io(inner) => OpError::Io(inner),
            other => OpError::Load(other.to_string()),
        }
    }
}
```

Add `pub mod ops;` to `crates/freerouting/src/lib.rs` after `pub mod mcp;`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p freerouting --lib ops::settings`
Expected: compile error, `SettingsOverrides`, `parse_set`, `resolve` not found.

- [ ] **Step 3: Implement**

Prepend to `crates/freerouting/src/ops/settings.rs`:

```rust
use std::path::PathBuf;

use fr_board::Board;
use fr_settings::sources::{CliSettings, JsonFileSettings};
use fr_settings::{CopyFields, HostEnvironment, RouterSettings, SettingsInputs, SettingsSource};

use super::OpError;

#[derive(Debug, Clone, Default)]
pub struct SettingsOverrides {
    pub settings_file: Option<PathBuf>,
    pub set: Vec<String>,
    pub sparse: Option<RouterSettings>,
}

pub fn parse_set(payload: &str) -> Result<(String, String), OpError> {
    let Some((name, value)) = payload.split_once('=') else {
        return Err(OpError::Settings(format!(
            "--set {payload}: expected <section>.<field>=<value>"
        )));
    };
    if !name.starts_with("router.") {
        return Err(OpError::Settings(format!(
            "--set {payload}: the field must start with `router.` (call `freerouting mcp`'s \
             list_settings, or see `--help`, for the names)"
        )));
    }
    Ok((name.to_string(), value.to_string()))
}

pub fn resolve(
    overrides: &SettingsOverrides,
    dsn: Option<&RouterSettings>,
    explicit_rules: Option<&[u8]>,
    scheduler_rules: Option<&[u8]>,
    board: Option<&Board>,
    host: &HostEnvironment,
) -> Result<RouterSettings, OpError> {
    let json_file = match overrides.settings_file.as_deref() {
        Some(path) => {
            let source = JsonFileSettings::new(path);
            if let Some(error) = source.errors().first() {
                return Err(OpError::Settings(format!(
                    "--settings {}: {error}",
                    path.display()
                )));
            }
            Some(source)
        }
        None => None,
    };

    let mut argv = Vec::with_capacity(overrides.set.len() * 2);
    for payload in &overrides.set {
        parse_set(payload)?;
        argv.push("--set".to_string());
        argv.push(payload.clone());
    }
    let cli = CliSettings::new_with_set_alias(&argv);
    if let Some(error) = cli.errors().first() {
        return Err(OpError::Settings(format!("--set: {error}")));
    }

    let inputs = SettingsInputs {
        json_file: json_file.as_ref().and_then(SettingsSource::get_settings),
        dsn,
        cli_rules: explicit_rules,
        scheduler_rules,
        env: None,
        cli: cli.get_settings(),
    };
    let mut settings = fr_settings::resolve_headless(&inputs, board, host);
    if let Some(sparse) = overrides.sparse.as_ref() {
        settings.apply_new_values_from(sparse);
        if let Some(board) = board {
            settings.apply_board_specific_optimizations(board);
        }
    }
    Ok(settings)
}
```

If `apply_new_values_from` is not reachable through `CopyFields`, open `crates/fr-settings/src/router_settings.rs`, find `fn apply_new_values_from`, and make it `pub fn`; then drop the `CopyFields` import. If `JsonFileSettings::errors()` is empty for a missing file, add a `std::fs::metadata(path)` check before constructing the source and return `OpError::Settings` when it fails.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p freerouting --lib ops::settings`
Expected: 5 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/freerouting/src/ops crates/freerouting/src/lib.rs
git commit -m "feat(cli): ops::settings — one settings ladder for the CLI and the MCP tools"
```

---

### Task 2: `ops::load` — the one load path

**Files:**
- Create: `crates/freerouting/src/ops/load.rs`
- Modify: `crates/freerouting/src/ops/mod.rs`
- Test: `crates/freerouting/tests/ops_load.rs`

**Interfaces:**
- Consumes: Task 1's `SettingsOverrides`, `resolve`, `OpError`.
- Produces: `BoardSource { Path(PathBuf), Text { text: String, name: String } }`; `LoadRequest { source, rules: Option<PathBuf>, discover_adjacent_rules: bool, session: Option<PathBuf>, kicad_project: Option<PathBuf>, settings: SettingsOverrides }`; `Loaded { job: RoutingJob, board: Board, transform: CoordinateTransform, metadata: Option<BoardMetadata>, settings: RouterSettings, warnings: Vec<String> }`; `load(&LoadRequest) -> Result<Loaded, OpError>`; `LoadRequest::for_board(source) -> LoadRequest` (all other fields empty, `discover_adjacent_rules: false`).

- [ ] **Step 1: Write the failing tests**

Create `crates/freerouting/tests/ops_load.rs`:

```rust
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use freerouting::ops::load::{BoardSource, LoadRequest, load};
use freerouting::ops::{OpError, SettingsOverrides};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("fr-ops-load").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

fn spike_dsn() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../benchmark/tests/data/spike/spike.dsn")
}

#[test]
fn a_dsn_path_loads_and_the_design_block_reaches_the_settings() {
    let loaded = load(&LoadRequest::for_board(BoardSource::Path(spike_dsn()))).unwrap();
    assert!(loaded.board.get_layer_count() >= 2);
    assert_eq!(
        loaded.settings.get_layer_count(),
        loaded.board.get_layer_count()
    );
    assert!(loaded.job.get_input().is_some());
}

#[test]
fn dsn_text_loads_under_the_given_name() {
    let text = std::fs::read_to_string(spike_dsn()).unwrap();
    let request = LoadRequest::for_board(BoardSource::Text {
        text,
        name: "board".to_string(),
    });
    let loaded = load(&request).unwrap();
    assert_eq!(loaded.job.name, "board");
    assert_eq!(loaded.job.get_input().unwrap().get_filename(), "board.dsn");
}

#[test]
fn a_missing_path_is_an_input_error() {
    let error = load(&LoadRequest::for_board(BoardSource::Path(PathBuf::from(
        "/nonexistent/board.dsn",
    ))))
    .unwrap_err();
    assert!(matches!(error, OpError::Input(_)), "{error}");
}

#[test]
fn a_session_file_is_not_a_board() {
    let dir = scratch("session-input");
    let input = dir.join("board.dsn");
    std::fs::write(&input, b"(session previous)\n").unwrap();
    let error = load(&LoadRequest::for_board(BoardSource::Path(input))).unwrap_err();
    assert!(matches!(error, OpError::Input(_)), "{error}");
    assert!(error.to_string().contains("Specctra DSN"), "{error}");
}

#[test]
fn an_explicit_rules_file_reaches_the_settings_and_an_adjacent_one_only_when_asked() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = scratch("rules");
    let dsn = dir.join("board.dsn");
    std::fs::copy(parity::fixture("Issue143-rpi_splitter.dsn"), &dsn).unwrap();
    std::fs::write(
        dir.join("board.rules"),
        b"(rules PCB board\n  (autoroute_settings\n    (via_costs 99)\n  )\n)\n",
    )
    .unwrap();

    let mut request = LoadRequest::for_board(BoardSource::Path(dsn.clone()));
    let quiet = load(&request).unwrap();
    assert_eq!(quiet.settings.scoring.as_ref().unwrap().via_costs, Some(50));

    request.discover_adjacent_rules = true;
    let discovered = load(&request).unwrap();
    assert_eq!(discovered.settings.scoring.as_ref().unwrap().via_costs, Some(99));

    request.discover_adjacent_rules = false;
    request.rules = Some(dir.join("board.rules"));
    let explicit = load(&request).unwrap();
    assert_eq!(explicit.settings.scoring.as_ref().unwrap().via_costs, Some(99));
}

#[test]
fn set_outranks_the_rules_file_and_the_sparse_payload_outranks_set() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = scratch("precedence");
    let dsn = dir.join("board.dsn");
    std::fs::copy(parity::fixture("Issue143-rpi_splitter.dsn"), &dsn).unwrap();
    let rules = dir.join("r.rules");
    std::fs::write(
        &rules,
        b"(rules PCB board\n  (autoroute_settings\n    (via_costs 99)\n  )\n)\n",
    )
    .unwrap();

    let mut request = LoadRequest::for_board(BoardSource::Path(dsn));
    request.rules = Some(rules);
    request.settings = SettingsOverrides {
        settings_file: None,
        set: vec!["router.scoring.via_costs=77".to_string()],
        sparse: None,
    };
    let loaded = load(&request).unwrap();
    assert_eq!(
        loaded.settings.scoring.as_ref().unwrap().via_costs,
        Some(99),
        "the rules file is re-applied after the flags; that is the resolver's order and it stays"
    );

    let mut sparse = fr_settings::RouterSettings::new();
    fr_settings::set_field_value(&mut sparse, "scoring.via_costs", "5").unwrap();
    request.settings.sparse = Some(sparse);
    let loaded = load(&request).unwrap();
    assert_eq!(loaded.settings.scoring.as_ref().unwrap().via_costs, Some(5));
}

#[test]
fn a_session_is_imported_after_the_rules_and_a_project_sets_constraints() {
    if !parity::require_java_dir() {
        return;
    }
    let dsn = parity::fixture("Issue593-BBD_Mars-64.dsn");
    let ses = parity::fixture("Issue593-BBD_Mars-64.ses");

    let bare = load(&LoadRequest::for_board(BoardSource::Path(dsn.clone()))).unwrap();
    let mut request = LoadRequest::for_board(BoardSource::Path(dsn));
    request.session = Some(ses);
    let with_session = load(&request).unwrap();
    assert!(
        with_session.board.get_traces().count() > bare.board.get_traces().count(),
        "the session's wires are on the board"
    );

    let project = spike_dsn().with_file_name("stripped.kicad_pro");
    let mut request = LoadRequest::for_board(BoardSource::Path(spike_dsn()));
    request.kicad_project = Some(project);
    let loaded = load(&request).unwrap();
    assert_eq!(
        loaded
            .board
            .rules
            .drc_constraints
            .as_ref()
            .and_then(|c| c.hole_to_hole),
        Some(2500)
    );
}
```

If `Board::get_traces()` returns something other than an iterator, use `.len()` on whatever it returns; the point is a count comparison.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p freerouting --test ops_load`
Expected: compile error, `freerouting::ops::load` not found.

- [ ] **Step 3: Implement**

Create `crates/freerouting/src/ops/load.rs`:

```rust
use std::path::{Path, PathBuf};

use fr_board::Board;
use fr_core::{FileFormat, RoutingJob, SessionId};
use fr_dsn::{BoardMetadata, CoordinateTransform};
use fr_settings::sources::DsnFileSettings;
use fr_settings::{HostEnvironment, RouterSettings, SettingsSource};

use super::settings::{self, SettingsOverrides};
use super::OpError;

#[derive(Debug, Clone)]
pub enum BoardSource {
    Path(PathBuf),
    Text { text: String, name: String },
}

#[derive(Debug, Clone)]
pub struct LoadRequest {
    pub source: BoardSource,
    pub rules: Option<PathBuf>,
    pub discover_adjacent_rules: bool,
    pub session: Option<PathBuf>,
    pub kicad_project: Option<PathBuf>,
    pub settings: SettingsOverrides,
}

impl LoadRequest {
    pub fn for_board(source: BoardSource) -> LoadRequest {
        LoadRequest {
            source,
            rules: None,
            discover_adjacent_rules: false,
            session: None,
            kicad_project: None,
            settings: SettingsOverrides::default(),
        }
    }
}

#[derive(Debug)]
pub struct Loaded {
    pub job: RoutingJob,
    pub board: Board,
    pub transform: CoordinateTransform,
    pub metadata: Option<BoardMetadata>,
    pub settings: RouterSettings,
    pub warnings: Vec<String>,
}

pub fn load(request: &LoadRequest) -> Result<Loaded, OpError> {
    let mut job = RoutingJob::new(SessionId::NIL);
    match &request.source {
        BoardSource::Path(path) => job.set_input(path).map_err(|error| {
            OpError::Input(format!(
                "Couldn't load the input file '{}': {error}",
                path.display()
            ))
        })?,
        BoardSource::Text { text, name } => {
            job.set_input_bytes(Some(text.as_bytes()));
            if let Some(input) = job.input.as_mut() {
                let extension = match input.format.default_extension() {
                    "" => "dsn",
                    extension => extension,
                };
                input.set_filename(Some(&format!("{name}.{extension}")));
            }
            job.name = name.clone();
        }
    }

    let input = job.get_input().expect("the input was just set");
    if !matches!(input.format, FileFormat::Dsn | FileFormat::KicadDesignJson) {
        return Err(OpError::Input(format!(
            "'{}' is not a board: only Specctra DSN and KiCad board JSON are accepted, got {}",
            input.get_filename(),
            input.format.java_name()
        )));
    }
    let dsn_source = DsnFileSettings::new(input.get_data(), input.get_filename());

    if let Some(rules) = request.rules.as_deref()
        && let Err(error) = job.set_rules(rules)
    {
        tracing::warn!("Couldn't load rules file '{}': {error}", rules.display());
    }
    let explicit_rules: Option<Vec<u8>> = job
        .rules
        .as_ref()
        .map(|rules| rules.get_data().to_vec())
        .filter(|data| !data.is_empty());
    let scheduler_rules = if request.discover_adjacent_rules {
        read_scheduler_rules(&job, request.rules.as_deref())
    } else {
        explicit_rules.clone()
    };

    let parsed = fr_core::parse_board_if_needed(&job)?;
    let mut board = parsed.board;
    let transform = parsed.transform;

    let host = HostEnvironment::detect();
    let mut settings = settings::resolve(
        &request.settings,
        dsn_source.get_settings(),
        explicit_rules.as_deref(),
        scheduler_rules.as_deref(),
        Some(&board),
        &host,
    )?;
    fr_core::apply_router_settings_for_loaded_board(&mut board, &mut settings);
    fr_core::apply_immediate_post_load_processing(&mut board);

    if let Some(bytes) = scheduler_rules.as_deref()
        && let Err(error) = fr_dsn::rules_reader::read(bytes, &job.name, &mut board, &transform, None)
    {
        tracing::error!("Failed to apply rules from rules file: {error}");
    }
    apply_kicad_project(request.kicad_project.as_deref(), &mut board, &transform);
    import_session(request.session.as_deref(), &mut board, &transform);

    job.router_settings = settings.clone();
    job.drc_settings = fr_settings::DesignRulesCheckerSettings::default();
    Ok(Loaded {
        job,
        board,
        transform,
        metadata: parsed.metadata,
        settings,
        warnings: parsed.warnings,
    })
}

fn read_scheduler_rules(job: &RoutingJob, explicit: Option<&Path>) -> Option<Vec<u8>> {
    if let Some(rules) = job.rules.as_ref() {
        let data = rules.get_data();
        if !data.is_empty() {
            return Some(data.to_vec());
        }
    }
    let dsn_path = job.get_input().and_then(|input| {
        (input.format == FileFormat::Dsn).then(|| PathBuf::from(input.get_absolute_path()))
    });
    let path = fr_settings::resolve_scheduler_rules_path(None, explicit, dsn_path.as_deref())?;
    match std::fs::read(&path) {
        Ok(bytes) => Some(bytes),
        Err(error) => {
            tracing::warn!("Failed to read rules file {}: {error}", path.display());
            None
        }
    }
}

fn apply_kicad_project(project: Option<&Path>, board: &mut Board, transform: &CoordinateTransform) {
    let Some(project) = project else {
        return;
    };
    let text = match std::fs::read_to_string(project) {
        Ok(text) => text,
        Err(error) => {
            tracing::warn!("KiCad project file {} not read: {error}", project.display());
            return;
        }
    };
    match fr_drc::apply_kicad_project(&text, board, transform) {
        Ok(()) => tracing::info!("KiCad project design rules loaded from {}", project.display()),
        Err(error) => tracing::error!("Failed to apply KiCad project design rules: {error}"),
    }
}

fn import_session(session: Option<&Path>, board: &mut Board, transform: &CoordinateTransform) {
    let Some(session) = session else {
        return;
    };
    let bytes = match std::fs::read(session) {
        Ok(bytes) => bytes,
        Err(error) => {
            tracing::warn!("Session file {} not read: {error}", session.display());
            return;
        }
    };
    if session.to_string_lossy().to_lowercase().ends_with(".json") {
        match fr_dsn::kicad::import_session(&String::from_utf8_lossy(&bytes), board) {
            Ok(()) => tracing::info!("KiCad JSON session loaded from {}", session.display()),
            Err(error) => tracing::error!("Failed to load session file: {error}"),
        }
        return;
    }
    match fr_dsn::ses_reader::read(&bytes[..], board, transform) {
        Ok(summary) => tracing::info!(
            "Session loaded from {}: {} wires, {} vias imported, {} errors",
            session.display(),
            summary.wires_imported,
            summary.vias_imported,
            summary.errors_encountered
        ),
        Err(error) => tracing::error!("Failed to load session file: {error}"),
    }
}
```

Add to `ops/mod.rs`: `pub mod load;` and `pub use load::{BoardSource, LoadRequest, Loaded, load};`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p freerouting --test ops_load`
Expected: 7 passed (or fewer plus skips when the corpus is absent).

- [ ] **Step 5: Commit**

```bash
git add crates/freerouting/src/ops crates/freerouting/tests/ops_load.rs
git commit -m "feat(cli): ops::load — one load path for route, drc, info and the MCP tools"
```

---

### Task 3: `ops::route`

**Files:**
- Create: `crates/freerouting/src/ops/route.rs`
- Modify: `crates/freerouting/src/ops/mod.rs`
- Test: `crates/freerouting/tests/ops_route.rs`

**Interfaces:**
- Consumes: Task 2's `LoadRequest`, `load`, `Loaded`.
- Produces: `OutputFormat { Ses, KicadSessionJson }` with `OutputFormat::from_path(&Path) -> Option<OutputFormat>`; `OutputTarget { File(PathBuf), Session }`; `RouteRequest { load, output, cancel: CancelToken, progress: SyncProgressSink, visualize: Option<RoutingVisualizationOptions> }`; `RouteOutcome { job, session: Vec<u8>, format, state: RoutingJobState, result: RoutingResult, visualization: Option<RoutingVisualizationSummary> }`; `route(RouteRequest) -> Result<RouteOutcome, OpError>`; `budget_for(&RouterSettings) -> RouterBudget`.

- [ ] **Step 1: Write the failing tests**

Create `crates/freerouting/tests/ops_route.rs`:

```rust
#![forbid(unsafe_code)]

use std::path::PathBuf;

use freerouting::ops::load::{BoardSource, LoadRequest};
use freerouting::ops::route::{OutputFormat, OutputTarget, RouteRequest, budget_for, route};
use freerouting::ops::{OpError, SettingsOverrides};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("fr-ops-route").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

fn request(dsn: PathBuf, output: OutputTarget, set: Vec<String>) -> RouteRequest {
    let mut load = LoadRequest::for_board(BoardSource::Path(dsn));
    load.discover_adjacent_rules = true;
    load.settings = SettingsOverrides {
        settings_file: None,
        set,
        sparse: None,
    };
    RouteRequest {
        load,
        output,
        cancel: fr_core::CancelToken::new(),
        progress: fr_core::SyncProgressSink::noop(),
        visualize: None,
    }
}

#[test]
fn output_format_is_the_extension() {
    assert_eq!(OutputFormat::from_path("a/b.ses".as_ref()), Some(OutputFormat::Ses));
    assert_eq!(OutputFormat::from_path("a/b.SES".as_ref()), Some(OutputFormat::Ses));
    assert_eq!(
        OutputFormat::from_path("b.json".as_ref()),
        Some(OutputFormat::KicadSessionJson)
    );
    assert_eq!(OutputFormat::from_path("b.dsn".as_ref()), None);
    assert_eq!(OutputFormat::from_path("b".as_ref()), None);
}

#[test]
fn the_budget_is_the_default_with_the_settings_knob() {
    let mut settings = fr_settings::RouterSettings::new();
    assert_eq!(budget_for(&settings).opt_changed_area_ms, 0);
    settings.opt_changed_area_ms = Some(250);
    assert_eq!(budget_for(&settings).opt_changed_area_ms, 250);
}

#[test]
fn a_routed_board_answers_a_session_and_its_stats() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = scratch("session");
    let outcome = route(request(
        parity::fixture("Issue143-rpi_splitter.dsn"),
        OutputTarget::File(dir.join("out.ses")),
        vec!["router.max_passes=1".to_string()],
    ))
    .unwrap();
    assert_eq!(outcome.format, OutputFormat::Ses);
    assert_eq!(outcome.state, fr_core::RoutingJobState::Completed);
    assert!(String::from_utf8_lossy(&outcome.session).starts_with("(session"));
    assert!(outcome.result.stats.connections.incomplete_count.is_some());
    assert_eq!(outcome.job.get_current_pass(), 1);
    assert!(
        !dir.join("out.ses").exists(),
        "the operation writes nothing; the adapter does"
    );
}

#[test]
fn a_json_output_carries_the_routed_board() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = scratch("json");
    let outcome = route(request(
        parity::fixture("Issue143-rpi_splitter.dsn"),
        OutputTarget::File(dir.join("out.json")),
        vec!["router.max_passes=1".to_string()],
    ))
    .unwrap();
    assert_eq!(outcome.format, OutputFormat::KicadSessionJson);
    let text = String::from_utf8_lossy(&outcome.session);
    assert!(text.contains("\"traces\""));
    assert!(!text.contains("\"traces\": []"));
}

#[test]
fn a_zero_job_timeout_reports_timed_out() {
    if !parity::require_java_dir() {
        return;
    }
    let outcome = route(request(
        parity::fixture("Issue143-rpi_splitter.dsn"),
        OutputTarget::Session,
        vec![
            "router.max_passes=1".to_string(),
            "router.job_timeout=0:00:00".to_string(),
        ],
    ))
    .unwrap();
    assert_eq!(outcome.state, fr_core::RoutingJobState::TimedOut);
    assert!(outcome.result.timed_out);
}

#[test]
fn a_bad_timeout_is_a_settings_error() {
    if !parity::require_java_dir() {
        return;
    }
    let error = route(request(
        parity::fixture("Issue143-rpi_splitter.dsn"),
        OutputTarget::Session,
        vec!["router.job_timeout=banana".to_string()],
    ))
    .unwrap_err();
    assert!(matches!(error, OpError::Settings(_)), "{error}");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p freerouting --test ops_route`
Expected: compile error, `freerouting::ops::route` not found.

- [ ] **Step 3: Implement**

Create `crates/freerouting/src/ops/route.rs`:

```rust
use std::path::{Path, PathBuf};
use std::time::Instant;

use fr_core::{
    BoardFileDetails, CancelToken, Ctx, FileFormat, JobStopReason, RouterBudget, RoutingJob,
    RoutingJobState, RoutingPipeline, RoutingResult, SyncProgressSink,
};
use fr_router::{RoutingVisualizationOptions, RoutingVisualizationSummary};
use fr_settings::RouterSettings;

use super::load::{LoadRequest, Loaded, load};
use super::OpError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Ses,
    KicadSessionJson,
}

impl OutputFormat {
    pub fn from_path(path: &Path) -> Option<OutputFormat> {
        let extension = path.extension()?.to_str()?.to_ascii_lowercase();
        match extension.as_str() {
            "ses" => Some(OutputFormat::Ses),
            "json" => Some(OutputFormat::KicadSessionJson),
            _ => None,
        }
    }

    fn file_format(self) -> FileFormat {
        match self {
            OutputFormat::Ses => FileFormat::Ses,
            OutputFormat::KicadSessionJson => FileFormat::KicadSessionJson,
        }
    }

    fn extension(self) -> &'static str {
        match self {
            OutputFormat::Ses => "ses",
            OutputFormat::KicadSessionJson => "json",
        }
    }
}

#[derive(Debug, Clone)]
pub enum OutputTarget {
    File(PathBuf),
    Session,
}

pub struct RouteRequest {
    pub load: LoadRequest,
    pub output: OutputTarget,
    pub cancel: CancelToken,
    pub progress: SyncProgressSink,
    pub visualize: Option<RoutingVisualizationOptions>,
}

#[derive(Debug)]
pub struct RouteOutcome {
    pub job: RoutingJob,
    pub session: Vec<u8>,
    pub format: OutputFormat,
    pub state: RoutingJobState,
    pub result: RoutingResult,
    pub visualization: Option<RoutingVisualizationSummary>,
}

pub fn budget_for(settings: &RouterSettings) -> RouterBudget {
    let mut budget = RouterBudget::default();
    if let Some(ms) = settings.opt_changed_area_ms {
        budget.opt_changed_area_ms = ms;
    }
    budget
}

pub fn route(request: RouteRequest) -> Result<RouteOutcome, OpError> {
    let format = match &request.output {
        OutputTarget::File(path) => OutputFormat::from_path(path).ok_or_else(|| {
            OpError::Input(format!(
                "'{}' is not an output file this program can write: the output must end in \
                 .ses (a Specctra session) or .json (a KiCad session)",
                path.display()
            ))
        })?,
        OutputTarget::Session => OutputFormat::Ses,
    };

    let Loaded {
        mut job,
        mut board,
        transform,
        settings,
        ..
    } = load(&request.load)?;

    if let OutputTarget::File(path) = &request.output {
        job.try_to_set_output_file(Some(path));
    }

    let cancel = match fr_core::job_timeout_deadline(settings.job_timeout_string.as_deref()) {
        Ok(Some(deadline)) => request.cancel.with_deadline_from(deadline),
        Ok(None) => request.cancel.clone(),
        Err(error) => {
            return Err(OpError::Settings(format!("router.job_timeout: {error}")));
        }
    };

    let visualization = match request.visualize {
        Some(options) => Some(
            fr_router::start_routing_visualization(options)
                .map_err(|error| OpError::Input(format!("--visualize: {error}")))?,
        ),
        None => None,
    };

    job.started_at = Some(Instant::now());
    job.state = RoutingJobState::Running;
    let ctx = Ctx {
        settings: &settings,
        cancel,
        progress: &request.progress,
        budget: budget_for(&settings),
    };
    let result = RoutingPipeline::run(&mut board, &ctx)?;
    let visualization = visualization.map(fr_router::RoutingVisualizationGuard::finish);

    job.finished_at = Some(Instant::now());
    job.set_current_pass(result.pipeline.router_passes_completed);
    job.set_optimizer_pass(result.pipeline.optimizer_passes_completed);
    let state = match result.stop_reason {
        Some(JobStopReason::Deadline) => RoutingJobState::TimedOut,
        Some(JobStopReason::Cancelled) => RoutingJobState::Cancelled,
        None => RoutingJobState::Completed,
    };
    job.state = state;

    let session = match format {
        OutputFormat::Ses => {
            let mut buffer = Vec::new();
            fr_core::save_as_specctra_session_ses(&board, &transform, &job.name, &mut buffer)?;
            buffer
        }
        OutputFormat::KicadSessionJson => fr_dsn::kicad::write(&board, &job.name).into_bytes(),
    };
    attach_output(&mut job, &session, format);

    Ok(RouteOutcome {
        job,
        session,
        format,
        state,
        result,
        visualization,
    })
}

fn attach_output(job: &mut RoutingJob, session: &[u8], format: OutputFormat) {
    let mut output = job.output.take().unwrap_or_default();
    if output.get_filename().is_empty() {
        let base = job.get_input().map_or_else(
            || job.name.clone(),
            BoardFileDetails::get_filename_without_extension,
        );
        output.filename = format!("{base}.{}", format.extension());
    }
    output.format = format.file_format();
    output.set_data(session.to_vec(), format.file_format());
    job.output = Some(output);
}
```

If `BoardFileDetails::filename` is not a public field, use `output.set_filename(Some(&format!("{base}.{}", format.extension())))` instead; for the `OutputTarget::Session` case the directory then comes from `set_filename`, which is what `route_board` answered before through `file_payload_fields`. Check the MCP conversation test in Task 6 still passes.

If `fr_core::Error` from `save_as_specctra_session_ses` does not convert with `?`, wrap it: `.map_err(OpError::from)?`.

Add to `ops/mod.rs`: `pub mod route;` and `pub use route::{OutputFormat, OutputTarget, RouteOutcome, RouteRequest, route};`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p freerouting --test ops_route`
Expected: all pass (corpus tests skip without the checkout).

- [ ] **Step 5: Commit**

```bash
git add crates/freerouting/src/ops crates/freerouting/tests/ops_route.rs
git commit -m "feat(cli): ops::route — load, run the pipeline, serialise the session"
```

---

### Task 4: `ops::drc` and `ops::info`

**Files:**
- Create: `crates/freerouting/src/ops/drc.rs`, `crates/freerouting/src/ops/info.rs`
- Modify: `crates/freerouting/src/ops/mod.rs`
- Test: `crates/freerouting/tests/ops_drc_info.rs`

**Interfaces:**
- Consumes: Task 2's `load`, Task 1's `resolve`.
- Produces: `DrcRequest { load, flavor: DrcJsonFlavor, date: String }`; `DrcOutcome { report: KiCadDrcReport, json: String, violation_count: usize }`; `drc(&DrcRequest) -> Result<DrcOutcome, OpError>`; `report_date(SystemTime) -> String` (moved from `commands/drc.rs`, unchanged); `InfoRequest { load }`; `info(&InfoRequest) -> Result<BoardSummary, OpError>`.

- [ ] **Step 1: Write the failing tests**

Create `crates/freerouting/tests/ops_drc_info.rs`:

```rust
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use fr_drc::report::DrcJsonFlavor;
use freerouting::ops::drc::{DrcRequest, drc, report_date};
use freerouting::ops::info::{InfoRequest, info};
use freerouting::ops::load::{BoardSource, LoadRequest};

fn spike_dsn() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../benchmark/tests/data/spike/spike.dsn")
}

#[test]
fn the_date_is_iso_offset_date_time_not_iso_instant() {
    use std::time::{Duration, UNIX_EPOCH};
    let base = 1_756_800_000u64;
    assert_eq!(report_date(UNIX_EPOCH + Duration::new(base, 0)), "2025-09-02T08:00Z");
    assert_eq!(
        report_date(UNIX_EPOCH + Duration::new(base, 120_000_000)),
        "2025-09-02T08:00:00.12Z"
    );
    assert_eq!(report_date(UNIX_EPOCH + Duration::new(base + 7, 0)), "2025-09-02T08:00:07Z");
}

#[test]
fn info_summarises_the_spike_board() {
    let summary = info(&InfoRequest {
        load: LoadRequest::for_board(BoardSource::Path(spike_dsn())),
    })
    .unwrap();
    let text = summary.to_json_pretty();
    assert!(text.contains("\"layers\""));
    assert!(text.contains("\"statistics\""));
}

#[test]
fn drc_counts_the_dev_boards_violations_in_both_flavors() {
    if !parity::require_java_dir() {
        return;
    }
    let dsn = parity::fixture("Issue575-drc_dev-board_4_hole_clearance_violations.dsn");
    let request = DrcRequest {
        load: LoadRequest::for_board(BoardSource::Path(dsn.clone())),
        flavor: DrcJsonFlavor::KiCad,
        date: "2025-09-02T08:00Z".to_string(),
    };
    let outcome = drc(&request).unwrap();
    assert_eq!(outcome.violation_count, 8);
    assert!(outcome.json.contains("\"quality_score\": 906.2450561523438"));
    assert!(outcome.json.contains("\"freerouting_version\": \"Freerouting "));
    assert!(outcome.json.contains(env!("CARGO_PKG_VERSION")));

    let head = DrcRequest {
        flavor: DrcJsonFlavor::FreeroutingHead,
        ..request
    };
    let outcome = drc(&head).unwrap();
    assert!(outcome.json.contains("\"qualityScore\""));
}

#[test]
fn a_rules_file_does_not_move_the_quality_score() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = std::env::temp_dir().join("fr-ops-drc");
    std::fs::create_dir_all(&dir).unwrap();
    let rules = dir.join("scoring.rules");
    std::fs::write(
        &rules,
        b"(rules PCB scoring\n  (autoroute_settings\n    (via_costs 999)\n  )\n)\n",
    )
    .unwrap();
    let dsn = parity::fixture("Issue575-drc_dev-board_4_hole_clearance_violations.dsn");
    let plain = drc(&DrcRequest {
        load: LoadRequest::for_board(BoardSource::Path(dsn.clone())),
        flavor: DrcJsonFlavor::KiCad,
        date: "d".to_string(),
    })
    .unwrap();
    let mut load = LoadRequest::for_board(BoardSource::Path(dsn));
    load.rules = Some(rules);
    let with_rules = drc(&DrcRequest {
        load,
        flavor: DrcJsonFlavor::KiCad,
        date: "d".to_string(),
    })
    .unwrap();
    assert_eq!(plain.report.quality_score, with_rules.report.quality_score);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p freerouting --test ops_drc_info`
Expected: compile error.

- [ ] **Step 3: Implement**

Create `crates/freerouting/src/ops/drc.rs`:

```rust
use fr_board::Board;
use fr_core::{BoardStatistics, RoutingJob};
use fr_drc::DesignRulesChecker;
use fr_drc::report::{DrcCoordinates, DrcJsonFlavor, DrcReportOptions, KiCadDrcReport};
use fr_settings::sources::DsnFileSettings;
use fr_settings::{HostEnvironment, SettingsSource};

use super::load::{LoadRequest, Loaded, load};
use super::settings::{self, SettingsOverrides};
use super::OpError;

pub struct DrcRequest {
    pub load: LoadRequest,
    pub flavor: DrcJsonFlavor,
    pub date: String,
}

#[derive(Debug)]
pub struct DrcOutcome {
    pub report: KiCadDrcReport,
    pub json: String,
    pub violation_count: usize,
}

pub fn drc(request: &DrcRequest) -> Result<DrcOutcome, OpError> {
    let Loaded {
        job,
        mut board,
        transform,
        ..
    } = load(&request.load)?;
    let source = job
        .get_input()
        .map_or_else(|| "board.dsn".to_string(), |input| input.get_filename().to_string());

    let coords = DrcCoordinates {
        board_unit: board.communication.unit,
        transform,
    };
    let options = DrcReportOptions {
        source,
        coordinate_unit: "mm".to_string(),
        date: request.date.clone(),
        freerouting_version: env!("CARGO_PKG_VERSION").to_string(),
        quality_score: None,
    };
    let mut report = DesignRulesChecker::new(&mut board).generate_report(&coords, &options);
    report.quality_score = quality_score(&mut board, &job, &request.load.settings).map(f64::from);

    let json = report.to_json(request.flavor).map_err(|error| {
        OpError::Load(format!("Couldn't serialise the DRC report: {error}"))
    })?;
    let violation_count = report.violations.len();
    Ok(DrcOutcome {
        report,
        json,
        violation_count,
    })
}

fn quality_score(board: &mut Board, job: &RoutingJob, overrides: &SettingsOverrides) -> Option<f32> {
    let input = job.get_input()?;
    let dsn = DsnFileSettings::new(input.get_data(), input.get_filename());
    let settings = settings::resolve(
        overrides,
        dsn.get_settings(),
        None,
        None,
        None,
        &HostEnvironment::detect(),
    )
    .ok()?;
    let scoring = settings.scoring.as_ref()?;
    Some(BoardStatistics::new(board).normalized_score(scoring))
}

pub fn report_date(time: std::time::SystemTime) -> String {
    let instant = fr_core::format_utc_iso8601(time);
    let body = instant.strip_suffix('Z').unwrap_or(&instant);
    let body = match body.split_once('.') {
        Some((head, fraction)) => {
            let trimmed = fraction.trim_end_matches('0');
            if trimmed.is_empty() {
                head.to_string()
            } else {
                format!("{head}.{trimmed}")
            }
        }
        None => body.to_string(),
    };
    let body = match body.strip_suffix(":00") {
        Some(without_seconds) if !body.contains('.') => without_seconds.to_string(),
        _ => body,
    };
    format!("{body}Z")
}
```

If the `quality_score` for the dev board does not print `906.2450561523438`, the resolver's settings differ from the merger-based one the old code used; compare `settings.scoring` between `fr_settings::resolve_headless` with `board: None` and a `SettingsMerger` built from `DefaultSettings` + `DsnFileSettings`, and use whichever reproduces the committed scores — the eight `tests/reference/drc-*/drc.json` values are the oracle, checked again in Task 5's `every_committed_reference_score_is_recomputed`.

Create `crates/freerouting/src/ops/info.rs`:

```rust
use fr_core::BoardSummary;

use super::load::{LoadRequest, load};
use super::OpError;

pub struct InfoRequest {
    pub load: LoadRequest,
}

pub fn info(request: &InfoRequest) -> Result<BoardSummary, OpError> {
    let mut loaded = load(&request.load)?;
    Ok(fr_core::summarise(&mut loaded.board, loaded.metadata.as_ref()))
}
```

Add to `ops/mod.rs`: `pub mod drc; pub mod info;` and `pub use drc::{DrcOutcome, DrcRequest, drc}; pub use info::{InfoRequest, info};`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p freerouting --test ops_drc_info`
Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add crates/freerouting/src/ops crates/freerouting/tests/ops_drc_info.rs
git commit -m "feat(cli): ops::drc and ops::info on the shared loader"
```

---

### Task 5: The native command line and the CLI adapters

**Files:**
- Modify: `crates/freerouting/src/cli.rs`, `crates/freerouting/src/lib.rs`, `crates/freerouting/src/commands/mod.rs`, `crates/freerouting/src/commands/route.rs`, `crates/freerouting/src/commands/drc.rs`, `crates/freerouting/src/commands/info.rs`, `crates/freerouting/src/logging.rs`
- Delete: `crates/freerouting/src/legacy.rs`, `crates/freerouting/tests/legacy_cli.rs`
- Test: `crates/freerouting/tests/cli_e2e.rs` (rewritten in Task 7; this task only keeps the crate compiling and the unit tests green)

**Interfaces:**
- Consumes: `ops::*` from Tasks 1–4.
- Produces: `freerouting::ExitCode { Ok = 0, Failure = 1, UsageError = 2 }` with `code()`; `commands::overrides(&Cli, &[String]) -> SettingsOverrides`; `commands::route::run(&Cli, &RouteArgs) -> ExitCode`, `commands::drc::run(&Cli, &DrcArgs) -> ExitCode`, `commands::info::run(&Cli, &InfoArgs) -> ExitCode`; `mcp::stdio::run(SettingsOverrides) -> i32` (Task 6 changes its body; this task changes only the call).

- [ ] **Step 1: Write the failing unit tests in `cli.rs`**

Replace the `#[cfg(test)] mod tests` in `crates/freerouting/src/cli.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_route_with_max_passes_timeout_and_set() {
        let cli = Cli::try_parse_from([
            "freerouting", "route", "a.dsn", "-o", "b.ses", "--max-passes", "3",
            "--timeout", "0:05:00", "--set", "router.scoring.via_costs=1",
        ])
        .unwrap();
        let Command::Route(r) = cli.command else { panic!("expected route") };
        assert_eq!(r.input, PathBuf::from("a.dsn"));
        assert_eq!(r.max_passes, Some(3));
        assert_eq!(r.timeout.as_deref(), Some("0:05:00"));
        assert_eq!(r.set, vec!["router.scoring.via_costs=1".to_string()]);
    }

    #[test]
    fn the_output_must_be_a_session_extension() {
        for bad in ["b.dsn", "b.txt", "b"] {
            let error = Cli::try_parse_from(["freerouting", "route", "a.dsn", "-o", bad])
                .expect_err(bad);
            assert_eq!(error.kind(), clap::error::ErrorKind::ValueValidation, "{bad}");
        }
        for good in ["b.ses", "b.json", "B.SES"] {
            Cli::try_parse_from(["freerouting", "route", "a.dsn", "-o", good]).expect(good);
        }
    }

    #[test]
    fn the_dead_flags_are_gone() {
        for flag in [
            "--threads", "--kicad-json", "--optimizer-improvement-threshold",
            "--update-strategy", "--hybrid-ratio", "--item-selection", "--ignore-net-classes",
        ] {
            let error =
                Cli::try_parse_from(["freerouting", "route", "a.dsn", "-o", "b.ses", flag, "1"])
                    .expect_err(flag);
            assert_eq!(error.kind(), clap::error::ErrorKind::UnknownArgument, "{flag}");
        }
        let error = Cli::try_parse_from(["freerouting", "-de", "a.dsn", "-do", "b.ses"])
            .expect_err("the legacy form");
        assert_eq!(error.kind(), clap::error::ErrorKind::UnknownArgument);
    }

    #[test]
    fn parses_bounded_routing_visualization() {
        let cli = Cli::try_parse_from([
            "freerouting", "route", "a.dsn", "-o", "b.ses", "--visualize", "frames",
            "--visualize-every", "25", "--visualize-max-frames", "80",
        ])
        .unwrap();
        let Command::Route(route) = cli.command else { panic!("expected route") };
        assert_eq!(route.visualize, Some(PathBuf::from("frames")));
        assert_eq!(route.visualize_every, 25);
        assert_eq!(route.visualize_max_frames, 80);
        assert_eq!((route.visualize_width, route.visualize_height), (1280, 720));
    }

    #[test]
    fn parses_drc_and_info_and_mcp() {
        let cli = Cli::try_parse_from([
            "freerouting", "drc", "a.dsn", "--kicad-project", "p.kicad_pro", "--schema", "freerouting",
        ])
        .unwrap();
        let Command::Drc(d) = cli.command else { panic!("expected drc") };
        assert_eq!(d.kicad_project, Some(PathBuf::from("p.kicad_pro")));
        assert_eq!(d.schema, DrcSchema::Freerouting);
        assert!(matches!(
            Cli::try_parse_from(["freerouting", "info", "a.dsn"]).unwrap().command,
            Command::Info(_)
        ));
        assert!(matches!(
            Cli::try_parse_from(["freerouting", "mcp"]).unwrap().command,
            Command::Mcp
        ));
    }

    #[test]
    fn settings_is_global_and_names_a_json_file() {
        let cli = Cli::try_parse_from([
            "freerouting", "--settings", "s.json", "info", "a.dsn",
        ])
        .unwrap();
        assert_eq!(cli.settings, Some(PathBuf::from("s.json")));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p freerouting --lib cli::tests`
Expected: FAIL — `timeout` is a `u64`, `--kicad-json` still parses, `-o b.dsn` still parses.

- [ ] **Step 3: Rewrite `cli.rs`**

Replace everything above the tests in `crates/freerouting/src/cli.rs` with:

```rust
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

use crate::ops::route::OutputFormat;

#[derive(Parser, Debug)]
#[command(
    name = "freerouting",
    version,
    propagate_version = true,
    about = "Headless PCB autorouter"
)]
pub struct Cli {
    /// Raise the log level: -v for debug, -vv for trace.
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,
    /// Set the log level outright: off, error, warn, info, debug, trace.
    #[arg(long, global = true)]
    pub log_level: Option<String>,
    /// A settings JSON, applied below the design's own settings and below --set.
    #[arg(long, global = true, value_name = "FILE")]
    pub settings: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Route a board and write the session.
    Route(RouteArgs),
    /// Check a board against its design rules and write the KiCad DRC report.
    Drc(DrcArgs),
    /// Print the board summary as JSON.
    Info(InfoArgs),
    /// Serve the routing tools over JSON-RPC on stdin/stdout.
    Mcp,
}

#[derive(Args, Debug)]
pub struct RouteArgs {
    /// A Specctra DSN or a KiCad board JSON.
    pub input: PathBuf,
    /// Where to write the session: a .ses (Specctra) or a .json (KiCad session).
    #[arg(short, long, value_parser = parse_output_path)]
    pub output: PathBuf,
    /// A Specctra .rules file. Without it, a .rules beside the input is used when present.
    #[arg(long)]
    pub rules: Option<PathBuf>,
    /// A session to import before routing, so the run continues from it.
    #[arg(long)]
    pub ses: Option<PathBuf>,
    /// A KiCad .kicad_pro whose design rules are applied before routing.
    #[arg(long)]
    pub kicad_project: Option<PathBuf>,
    /// How many routing passes to run; 0 means unlimited.
    #[arg(long)]
    pub max_passes: Option<u32>,
    /// Wall-clock budget for the whole job, as hh:mm:ss or seconds.
    #[arg(long)]
    pub timeout: Option<String>,
    /// Write the result manifest here.
    #[arg(long)]
    pub result_json: Option<PathBuf>,
    /// Override one setting: --set router.<section>.<field>=<value>. Repeatable.
    /// router.max_items=N stops the optimizer as well as the router; --max-passes stops
    /// only the router.
    #[arg(long = "set", value_name = "FIELD=VALUE")]
    pub set: Vec<String>,
    /// Write SVG frames showing maze rooms over the routed PCB.
    #[arg(long, value_name = "DIR")]
    pub visualize: Option<PathBuf>,
    /// Capture one visualization frame for every N maze steps; route commits are always kept.
    #[arg(long, default_value_t = 1, requires = "visualize")]
    pub visualize_every: u64,
    /// Stop writing visualization frames after this many images; routing continues.
    #[arg(long, default_value_t = 1_000, requires = "visualize")]
    pub visualize_max_frames: u64,
    #[arg(long, default_value_t = 1280, requires = "visualize")]
    pub visualize_width: u32,
    #[arg(long, default_value_t = 720, requires = "visualize")]
    pub visualize_height: u32,
}

#[derive(Args, Debug)]
pub struct DrcArgs {
    /// A Specctra DSN or a KiCad board JSON.
    pub input: PathBuf,
    /// A session to apply before checking, so the report describes the routed board.
    #[arg(long)]
    pub ses: Option<PathBuf>,
    /// A Specctra .rules file to apply before checking.
    #[arg(long)]
    pub rules: Option<PathBuf>,
    /// A KiCad .kicad_pro whose design rules are applied before checking.
    #[arg(long)]
    pub kicad_project: Option<PathBuf>,
    /// Where to write the report; stdout when omitted.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    /// Which spelling of the KiCad DRC schema to write.
    #[arg(long, value_enum, default_value_t = DrcSchema::Kicad)]
    pub schema: DrcSchema,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DrcSchema {
    #[default]
    Kicad,
    Freerouting,
}

#[derive(Args, Debug)]
pub struct InfoArgs {
    /// A Specctra DSN or a KiCad board JSON.
    pub input: PathBuf,
}

fn parse_output_path(value: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(value);
    match OutputFormat::from_path(&path) {
        Some(_) => Ok(path),
        None => Err(format!(
            "'{value}' must end in .ses (a Specctra session) or .json (a KiCad session)"
        )),
    }
}
```

- [ ] **Step 4: Rewrite `lib.rs`**

Replace `crates/freerouting/src/lib.rs` with:

```rust
#![forbid(unsafe_code)]

pub mod cli;
pub mod commands;
pub mod logging;
pub mod mcp;
pub mod ops;

use clap::Parser;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ExitCode {
    Ok = 0,
    Failure = 1,
    UsageError = 2,
}

impl ExitCode {
    #[must_use]
    pub fn code(self) -> i32 {
        self as i32
    }
}

#[must_use]
pub fn run(raw: &[String]) -> ExitCode {
    let cli = match cli::Cli::try_parse_from(
        std::iter::once("freerouting".to_string()).chain(raw.iter().cloned()),
    ) {
        Ok(cli) => cli,
        Err(error) => {
            let _ = error.print();
            return if error.use_stderr() {
                ExitCode::UsageError
            } else {
                ExitCode::Ok
            };
        }
    };
    logging::init(logging::level_for(cli.verbose, cli.log_level.as_deref()));

    match &cli.command {
        cli::Command::Route(args) => commands::route::run(&cli, args),
        cli::Command::Drc(args) => commands::drc::run(&cli, args),
        cli::Command::Info(args) => commands::info::run(&cli, args),
        cli::Command::Mcp => match mcp::stdio::run(commands::overrides(&cli, &[])) {
            0 => ExitCode::Ok,
            _ => ExitCode::Failure,
        },
    }
}
```

- [ ] **Step 5: Trim `logging.rs`**

Replace `console_level_string` and `level_from_argv` (and the module doc line above `use`) in `crates/freerouting/src/logging.rs` with:

```rust
#[must_use]
pub fn level_for(verbosity: u8, log_level: Option<&str>) -> LogLevel {
    let level = log_level.map_or(LogLevel::Info, LogLevel::parse_name);
    match verbosity {
        0 => level,
        1 => level.max(LogLevel::Debug),
        _ => LogLevel::Trace,
    }
}
```

Rename `LogLevel::parse_java` to `parse_name` and remove the `/// `Level.valueOf` …` comment above it. Delete `MESSAGE_MAP` and its `#[cfg(test)] mod tests` block (everything from `pub const MESSAGE_MAP` to the end of the file), then add:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbosity_raises_the_level_and_an_unknown_name_is_info() {
        assert_eq!(level_for(0, None), LogLevel::Info);
        assert_eq!(level_for(0, Some("warn")), LogLevel::Warn);
        assert_eq!(level_for(0, Some("nonsense")), LogLevel::Info);
        assert_eq!(level_for(1, Some("error")), LogLevel::Debug);
        assert_eq!(level_for(2, None), LogLevel::Trace);
    }
}
```

- [ ] **Step 6: Rewrite `commands/mod.rs`**

```rust
pub mod drc;
pub mod info;
pub mod route;

use crate::cli::Cli;
use crate::ops::SettingsOverrides;

pub fn overrides(cli: &Cli, set: &[String]) -> SettingsOverrides {
    SettingsOverrides {
        settings_file: cli.settings.clone(),
        set: set.to_vec(),
        sparse: None,
    }
}
```

- [ ] **Step 7: Rewrite `commands/route.rs`**

```rust
use std::path::Path;

use fr_core::{RoutingJobState, RoutingResultManifest, SyncProgressSink};

use crate::ExitCode;
use crate::cli::{Cli, RouteArgs};
use crate::ops::load::{BoardSource, LoadRequest};
use crate::ops::route::{OutputTarget, RouteOutcome, RouteRequest, route};

pub fn run(cli: &Cli, args: &RouteArgs) -> ExitCode {
    let mut set = args.set.clone();
    if let Some(passes) = args.max_passes {
        set.push(format!("router.max_passes={passes}"));
    }
    if let Some(timeout) = args.timeout.as_deref() {
        set.push(format!("router.job_timeout={timeout}"));
    }

    let mut load = LoadRequest::for_board(BoardSource::Path(args.input.clone()));
    load.rules = args.rules.clone();
    load.discover_adjacent_rules = true;
    load.session = args.ses.clone();
    load.kicad_project = args.kicad_project.clone();
    load.settings = super::overrides(cli, &set);

    let visualize = args.visualize.as_ref().map(|dir| {
        let mut options = fr_router::RoutingVisualizationOptions::new(dir.clone());
        options.every = args.visualize_every;
        options.max_frames = args.visualize_max_frames;
        options.width = args.visualize_width;
        options.height = args.visualize_height;
        options
    });

    let outcome = match route(RouteRequest {
        load,
        output: OutputTarget::File(args.output.clone()),
        cancel: fr_core::CancelToken::new(),
        progress: SyncProgressSink::noop(),
        visualize,
    }) {
        Ok(outcome) => outcome,
        Err(error) => {
            tracing::error!("{error}");
            return ExitCode::Failure;
        }
    };

    if let Some(summary) = outcome.visualization.as_ref() {
        if let Some(error) = summary.error.as_ref() {
            tracing::warn!("routing visualization stopped after an I/O error: {error}");
        }
        eprintln!(
            "Visualization: {} SVG frames from {} routing steps in {}{}",
            summary.frames_written,
            summary.observed_steps,
            summary.output_dir.display(),
            if summary.reached_frame_limit { " (frame limit reached)" } else { "" }
        );
    }

    let written = write_session(&outcome, &args.output);
    let exit_code = if written
        && matches!(outcome.state, RoutingJobState::Completed | RoutingJobState::TimedOut)
    {
        ExitCode::Ok
    } else {
        ExitCode::Failure
    };
    write_manifest(cli, args, &outcome, written, exit_code);
    exit_code
}

fn write_session(outcome: &RouteOutcome, path: &Path) -> bool {
    if !matches!(outcome.state, RoutingJobState::Completed | RoutingJobState::TimedOut) {
        return false;
    }
    if let Err(error) = std::fs::write(path, &outcome.session) {
        tracing::error!("Couldn't save the output file '{}': {error}", path.display());
        return false;
    }
    !outcome.session.is_empty()
}

fn write_manifest(
    cli: &Cli,
    args: &RouteArgs,
    outcome: &RouteOutcome,
    written: bool,
    exit_code: ExitCode,
) {
    let _ = cli;
    let from_settings = outcome
        .job
        .router_settings
        .result_json_path
        .as_deref()
        .map(str::to_string)
        .filter(|path| !path.trim().is_empty());
    let Some(path) = from_settings.or_else(|| {
        args.result_json
            .as_deref()
            .map(|path| path.to_string_lossy().into_owned())
    }) else {
        return;
    };
    let manifest = RoutingResultManifest::from_job(
        &outcome.job,
        Some(&args.input),
        written,
        exit_code.code(),
        &fr_core::now_utc_iso8601,
        Some(&outcome.result.stats),
    );
    if let Err(error) = RoutingResultManifest::write(Path::new(&path), &manifest) {
        tracing::error!("Couldn't write routing result manifest to '{path}': {error}");
    }
}
```

Drop the `let _ = cli;` line and the `cli` parameter of `write_manifest` if clippy complains about the unused parameter; it is there only so the signature mirrors `run`'s.

- [ ] **Step 8: Rewrite `commands/drc.rs`**

```rust
use std::path::Path;

use fr_drc::report::DrcJsonFlavor;

use crate::ExitCode;
use crate::cli::{Cli, DrcArgs, DrcSchema};
use crate::ops::drc::{DrcRequest, drc, report_date};
use crate::ops::load::{BoardSource, LoadRequest};

pub fn run(cli: &Cli, args: &DrcArgs) -> ExitCode {
    let mut load = LoadRequest::for_board(BoardSource::Path(args.input.clone()));
    load.rules = args.rules.clone();
    load.session = args.ses.clone();
    load.kicad_project = args.kicad_project.clone();
    load.settings = super::overrides(cli, &[]);

    let outcome = match drc(&DrcRequest {
        load,
        flavor: match args.schema {
            DrcSchema::Kicad => DrcJsonFlavor::KiCad,
            DrcSchema::Freerouting => DrcJsonFlavor::FreeroutingHead,
        },
        date: report_date(std::time::SystemTime::now()),
    }) {
        Ok(outcome) => outcome,
        Err(error) => {
            tracing::error!("{error}");
            return ExitCode::Failure;
        }
    };

    match args.output.as_deref() {
        None => println!("{}", outcome.json),
        Some(path) => {
            if let Err(error) = write_report(path, &outcome.json) {
                tracing::error!("Couldn't save the DRC report to '{}': {error}", path.display());
                return ExitCode::Failure;
            }
            tracing::info!("DRC report written to {}", path.display());
        }
    }
    if outcome.violation_count > 0 {
        tracing::warn!("{} DRC violation(s) found", outcome.violation_count);
        ExitCode::Failure
    } else {
        ExitCode::Ok
    }
}

fn write_report(path: &Path, json: &str) -> std::io::Result<()> {
    std::fs::write(path, json.as_bytes())
}
```

- [ ] **Step 9: Rewrite `commands/info.rs`**

```rust
use crate::ExitCode;
use crate::cli::{Cli, InfoArgs};
use crate::ops::info::{InfoRequest, info};
use crate::ops::load::{BoardSource, LoadRequest};

pub fn run(cli: &Cli, args: &InfoArgs) -> ExitCode {
    let mut load = LoadRequest::for_board(BoardSource::Path(args.input.clone()));
    load.settings = super::overrides(cli, &[]);
    match info(&InfoRequest { load }) {
        Ok(summary) => {
            println!("{}", summary.to_json_pretty());
            ExitCode::Ok
        }
        Err(error) => {
            tracing::error!("{error}");
            ExitCode::Failure
        }
    }
}
```

- [ ] **Step 10: Delete the legacy form and make the MCP call compile**

```bash
git rm -q crates/freerouting/src/legacy.rs crates/freerouting/tests/legacy_cli.rs
```

In `crates/freerouting/src/mcp/stdio.rs` change `pub fn run(settings_argv: &[String]) -> i32` to `pub fn run(overrides: crate::ops::SettingsOverrides) -> i32` and its first line to `let mut state = State::with_overrides(overrides);`. In `crates/freerouting/src/mcp/server.rs` replace the `settings_argv: Vec<String>` field with `pub overrides: crate::ops::SettingsOverrides`, `settings_argv: Vec::new()` with `overrides: crate::ops::SettingsOverrides::default()`, and `with_settings_argv` with:

```rust
    pub fn with_overrides(overrides: crate::ops::SettingsOverrides) -> Self {
        Self {
            overrides,
            ..Self::new()
        }
    }
```

The three tool files still reference `state.settings_argv` and `crate::commands::route::…`; Task 6 rewrites them. Until then, make them compile by replacing each `state.settings_argv` read with `state.overrides.clone()` where a `SettingsOverrides` is needed and deleting the `crate::commands::*` imports, or — simpler — do Task 6 immediately after this step and run the build once at the end of Task 6. Either way, **do not commit a tree that does not build.** The recommended order is: finish this task's steps 1–10, then Task 6, then run the checks and commit both together as two commits by staging the files of each task separately.

- [ ] **Step 11: Build and run the unit tests**

Run: `cargo build -p freerouting && cargo test -p freerouting --lib`
Expected: builds; `cli::tests` and `logging::tests` pass.

- [ ] **Step 12: Commit**

```bash
git add -A crates/freerouting/src crates/freerouting/tests/legacy_cli.rs
git commit -m "feat(cli): native command line only — the subcommands are adapters over ops"
```

---

### Task 6: The MCP adapters

**Files:**
- Modify: `crates/freerouting/src/mcp/tools/route_board.rs`, `check_drc.rs`, `board_info.rs`, `mod.rs`, `crates/freerouting/src/mcp/server.rs`, `crates/freerouting/src/mcp/stdio.rs`
- Test: `crates/freerouting/tests/mcp_stdio.rs`

**Interfaces:**
- Consumes: `ops::*`; `State::overrides`.
- Produces: unchanged MCP wire surface (the tool list, schemas and result shapes are the same).

- [ ] **Step 1: Delete the env-var test**

In `crates/freerouting/tests/mcp_stdio.rs` delete the whole `an_unreadable_router_budget_is_an_error_and_the_server_survives` test, and delete `Pipes::start_with_env` by folding its body into `Pipes::start()` (no `env` loop). Run: `cargo test -p freerouting --test mcp_stdio` — expected: compile failures in the tools until Step 2.

- [ ] **Step 2: Rewrite `mcp/tools/mod.rs`'s board input helper**

Replace `board_input` (and remove the `RoutingJob`, `SessionId`, `Uuid128` imports it used, keeping `mint_job_id`) with:

```rust
pub fn board_source(args: &Value) -> Result<crate::ops::load::BoardSource, RpcError> {
    let path = optional_string(args, "dsn_path")?;
    let text = optional_string(args, "dsn_text")?;
    match (path, text) {
        (Some(_), Some(_)) => Err(RpcError::invalid_params(
            "give exactly one of dsn_path and dsn_text, not both",
        )),
        (None, None) => Err(RpcError::invalid_params(
            "one of dsn_path and dsn_text is required",
        )),
        (Some(path), None) => Ok(crate::ops::load::BoardSource::Path(PathBuf::from(path))),
        (None, Some(text)) => Ok(crate::ops::load::BoardSource::Text {
            text,
            name: "board".to_string(),
        }),
    }
}

pub fn rpc_error(error: crate::ops::OpError) -> RpcError {
    use crate::ops::OpError;
    match error {
        OpError::Input(message) | OpError::Settings(message) => RpcError::invalid_params(message),
        other => RpcError::internal(other.to_string()),
    }
}
```

- [ ] **Step 3: Rewrite `route_board.rs`**

Keep `settings_payload` and `progress_sink` as they are. Replace `run` and delete `prototype_merger`:

```rust
pub fn run(
    state: &State,
    args: Value,
    progress: &ProgressWriter,
    cancel: &CancelToken,
) -> Result<Value, RpcError> {
    let ses = super::optional_string(&args, "ses_path")?.map(PathBuf::from);
    let rules = super::optional_string(&args, "rules_path")?.map(PathBuf::from);
    let output_path = super::optional_string(&args, "output_path")?.map(PathBuf::from);
    let payload = settings_payload(&args)?;

    let mut load = LoadRequest::for_board(super::board_source(&args)?);
    load.rules = rules;
    load.discover_adjacent_rules = true;
    load.session = ses;
    load.settings = SettingsOverrides {
        sparse: payload,
        ..state.overrides.clone()
    };

    let mut outcome = route(RouteRequest {
        load,
        output: OutputTarget::Session,
        cancel: cancel.clone(),
        progress: progress_sink(progress),
        visualize: None,
    })
    .map_err(super::rpc_error)?;
    outcome.job.id = super::mint_job_id();

    let output = outcome
        .job
        .output
        .as_ref()
        .ok_or_else(|| RpcError::internal("the router produced no session document"))?;
    let mut answer = super::file_payload_fields(output);
    let object = answer.as_object_mut().expect("a JSON object");
    object.insert("job_id".into(), json!(outcome.job.id.to_java_string()));
    match output_path.as_deref() {
        Some(path) => {
            std::fs::write(path, &outcome.session).map_err(|error| {
                RpcError::internal(format!(
                    "Couldn't save the output file '{}': {error}",
                    path.display()
                ))
            })?;
            object.insert("ses_path".into(), json!(path.display().to_string()));
        }
        None => {
            object.insert(
                "ses_text".into(),
                json!(String::from_utf8_lossy(&outcome.session).into_owned()),
            );
            object.insert("data".into(), json!(super::base64_encode(&outcome.session)));
        }
    }
    object.insert("stats".into(), fr_core::to_gson_json(&outcome.result.stats));
    object.insert("incompletes".into(), json!(outcome.result.incomplete_count()));
    object.insert("unrouted_report".into(), json!(outcome.result.unrouted_report));
    object.insert("drc_violation_count".into(), json!(outcome.result.violation_count()));
    object.insert("timed_out".into(), json!(outcome.result.timed_out));
    Ok(answer)
}
```

Imports for the file: `use crate::ops::SettingsOverrides; use crate::ops::load::LoadRequest; use crate::ops::route::{OutputTarget, RouteRequest, route}; use fr_core::{CancelToken, RouterSettings? no — RouterSettings comes from fr_settings}` — keep `fr_settings::RouterSettings` for `settings_payload`, and drop the `fr_settings::sources::*`, `SettingsMerger`, `HostEnvironment`, `FileFormat`, `JobStopReason`, `RoutingJobState`, `RoutingPipeline`, `Ctx` imports. `RoutingJob.id` is a public field.

- [ ] **Step 4: Rewrite `check_drc.rs`**

```rust
use super::super::jsonrpc::RpcError;
use super::super::server::{ProgressWriter, State};
use crate::ops::drc::{DrcRequest, drc, report_date};
use crate::ops::load::LoadRequest;
use fr_core::CancelToken;
use fr_drc::report::DrcJsonFlavor;
use serde_json::Value;
use std::path::PathBuf;

pub fn run(
    state: &State,
    args: Value,
    _progress: &ProgressWriter,
    _cancel: &CancelToken,
) -> Result<Value, RpcError> {
    let mut load = LoadRequest::for_board(super::board_source(&args)?);
    load.session = super::optional_string(&args, "ses_path")?.map(PathBuf::from);
    load.rules = super::optional_string(&args, "rules_path")?.map(PathBuf::from);
    load.kicad_project = super::optional_string(&args, "kicad_project_path")?.map(PathBuf::from);
    load.settings = state.overrides.clone();

    let outcome = drc(&DrcRequest {
        load,
        flavor: DrcJsonFlavor::KiCad,
        date: report_date(std::time::SystemTime::now()),
    })
    .map_err(super::rpc_error)?;
    serde_json::from_str(&outcome.json).map_err(|error| {
        RpcError::internal(format!("the DRC report is not readable as JSON: {error}"))
    })
}
```

- [ ] **Step 5: Rewrite `board_info.rs`**

```rust
use super::super::jsonrpc::RpcError;
use super::super::server::{ProgressWriter, State};
use crate::ops::info::{InfoRequest, info};
use crate::ops::load::LoadRequest;
use fr_core::CancelToken;
use serde_json::Value;

pub fn run(
    state: &State,
    args: Value,
    _progress: &ProgressWriter,
    _cancel: &CancelToken,
) -> Result<Value, RpcError> {
    let mut load = LoadRequest::for_board(super::board_source(&args)?);
    load.settings = state.overrides.clone();
    let summary = info(&InfoRequest { load }).map_err(super::rpc_error)?;
    serde_json::to_value(&summary).map_err(|error| {
        RpcError::internal(format!("the board summary would not serialize: {error}"))
    })
}
```

- [ ] **Step 6: Build and run the MCP tests**

Run: `cargo build -p freerouting && cargo test -p freerouting --test mcp_stdio`
Expected: all pass. If `the_four_tools_over_spawned_pipes` fails on `filename`/`path`, the `attach_output` branch in Task 3 needs `set_filename` (see the note there) so the input's directory is reported.

- [ ] **Step 7: Commit**

```bash
git add crates/freerouting/src/mcp crates/freerouting/tests/mcp_stdio.rs
git commit -m "feat(mcp): the tools are adapters over ops"
```

---

### Task 7: `cli_e2e.rs` on the native form

**Files:**
- Modify: `crates/freerouting/tests/cli_e2e.rs`, `tests/parity/src/lib.rs`, `tests/reference/cli-*/argv.txt` (13 files), `tests/reference/cli-fixtures.txt`
- Delete: `tests/reference/cli-*/route.log` (13 files)

**Interfaces:**
- Consumes: the binary from Tasks 5–6.
- Produces: `parity::cli_argv` unchanged in signature; `parity::normalize_manifest` also drops `app_version`; `parity::normalize_log`, `run_jar`, `jar_path`, `java_binary` and the `LogLine`/projection types removed.

- [ ] **Step 1: Rewrite the argv files**

For each of the 13 `tests/reference/cli-<stem>/argv.txt`, translate the tokens: `-de X` → `route X`, `-do Y` → `-o Y`, `-mp N` → `--max-passes N`, `--router.a.b=c` → `--set router.a.b=c`, one token per line. Run this once:

```bash
for f in tests/reference/cli-*/argv.txt; do
  python3 - "$f" <<'EOF'
import sys
p = sys.argv[1]
toks = [t for t in open(p).read().split("\n") if t]
out = []
i = 0
while i < len(toks):
    t = toks[i]
    if t == "-de": out += ["route", toks[i+1]]; i += 2
    elif t == "-do": out += ["-o", toks[i+1]]; i += 2
    elif t == "-mp": out += ["--max-passes", toks[i+1]]; i += 2
    elif t.startswith("--router."): out += ["--set", t[2:]]; i += 1
    else: raise SystemExit(f"{p}: unexpected token {t}")
open(p, "w").write("\n".join(out) + "\n")
EOF
done
cat tests/reference/cli-router-rpi-splitter/argv.txt
```

Expected last output:

```
route
<JAVA_DIR>/fixtures/Issue143-rpi_splitter.dsn
-o
<OUT>/route.ses
--max-passes
8
--set
router.fanout.enabled=true
--set
router.optimizer.enabled=true
```

If `python3` is unavailable, make the same edit by hand in the 13 files.

In `tests/reference/cli-fixtures.txt`, replace the header comment with the three lines below and rewrite the `extra_args` column of every row the same way (`-mp 8 --router.fanout.enabled=true …` becomes `--max-passes 8 --set router.fanout.enabled=true …`; `-` stays `-`):

```
# CLI end-to-end reference stems for crates/freerouting/tests/cli_e2e.rs.
#   stem|dsn|extra_args|lane
# dsn is relative to the fixture corpus; extra_args is what follows `route <dsn> -o <out>`, or `-`;
# lane is ci or slow. Goldens are cut with FR_REGOLDEN=<label> cargo test -p freerouting --test cli_e2e.
```

Delete the log goldens: `git rm -q tests/reference/cli-*/route.log`.

- [ ] **Step 2: Trim `tests/parity/src/lib.rs`**

Delete `normalize_log` and everything it alone uses (the `LogLine`-style projection types and helpers between the `// normalize_log` banner and `normalize_manifest`), `run_jar`, `jar_path`, `java_binary`. Keep `run_port`, `run_port_binary`, `cli_stems`, `cli_reference`, `cli_argv`, `normalize_manifest`, `normalize_ses_head_tokens`, `regolden_label`, `declared_lane`, `assert_port_lane_provenance`. Update `CliStem.extra`'s doc comment to `/// Everything after `route <dsn> -o <out>`, already split; empty for a bare run.` In `normalize_manifest`, add `object.remove("app_version");` after `object.remove("git_sha");`.

Run: `cargo build -p parity` — expected: builds. Then `grep -rn 'normalize_log\|run_jar\|jar_path\|java_binary' crates tests/parity/src` — expected: only `cli_e2e.rs` (fixed next).

- [ ] **Step 3: Rewrite `cli_e2e.rs`**

Keep the helpers (`scratch`, `stage_dsn`, `small_dsn`, `run`, `final_state`, `settings_snapshot`, `json_array_len`, `drc_dsn`, `report`, `autoroute_settings_rules`, `stable_manifest`, `climb`, `regolden_one`) and rewrite the tests as follows. Every legacy argv becomes the native one: `-de X -do Y` → `route X -o Y`; `-mp N` → `--max-passes N`; `--router.result_json=F` → `--result-json F`; `--router.a.b=c` → `--set router.a.b=c`; a session in the `-de` slot list → `--ses`.

Tests to keep with translated argv and unchanged assertions: `a_failed_run_leaves_the_previous_result_on_disk`, `an_empty_output_directory_is_not_unlinked`, `do_out_json_writes_the_routed_board`, `a_non_ascii_session_file_is_read_as_utf8` (the session moves to `--ses`), `a_missing_dr_path_disables_rules_discovery` (`-dr` → `--rules`), `no_donation_banner_on_stdout`, `max_passes_zero_is_unlimited`, `the_rules_file_is_read_as_bytes_twice`, `the_via_net_number_fixture_routes_instead_of_hanging`, `the_session_is_imported_after_the_rules` (its `build` helper now uses `freerouting::ops::load` with `rules`/`session` set, and the two orderings are expressed as two `LoadRequest`s: one with both, one with only the session then a second `load` cannot reorder — so replace the `rules_first`/`session_first` comparison with a single assertion that the CLI's clearance count equals the count from a `LoadRequest` carrying both, and keep the log-order assertion on `Loading RULES`/`Session loaded`; use the new log wording from Task 2: `Session loaded from`), `the_quality_score_uses_a_dsn_only_merge`, `the_quality_score_is_an_f32_widened_to_f64`, `every_committed_reference_score_is_recomputed` (append `let _ = code;` and drop the `assert_eq!(code, 0)` — the dev board and others now exit 1 on violations; assert instead `code == 0 || code == 1`), `drc_with_no_output_prints_to_stdout` (the stderr assertion becomes `stderr.contains("violation")` since the dev board has violations and the report still goes to stdout; exit code is 1), `info_writes_the_board_summary_to_stdout`, `info_exits_1_on_an_unreadable_input_and_on_an_unloadable_board` (the message assertion becomes `stderr.contains("Couldn't load the input file")` for the first and `stderr.contains("not a board")` for the second), `the_cli_passes_kicad_flavor_explicitly` (exit codes become 1), `two_runs_of_every_ci_stem_are_byte_identical` (`--router.result_json=` → `--result-json`), `every_stem_has_a_reference_and_every_reference_has_a_stem` (drop `"route.log"` from the list), `the_cli_reference_agrees_with_the_batch_reference`.

Tests to rewrite:

```rust
#[test]
fn an_unsupported_output_extension_is_refused_at_the_argument() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = scratch("unsupported-output-extension");
    let dsn = small_dsn().to_string_lossy().into_owned();
    for name in ["out.dsn", "out.scr", "out.frb", "out.txt", "out.rules", "out"] {
        let output = dir.join(name);
        let (_, stderr, code) = run(&["route", &dsn, "-o", &output.to_string_lossy()]);
        assert_eq!(code, 2, "-o {name} is a usage error");
        assert!(stderr.contains(".ses") && stderr.contains(".json"), "{stderr}");
        assert!(!output.exists(), "-o {name}: nothing may be created");
    }
    for name in ["out.ses", "out.json"] {
        let output = dir.join(name);
        let (_, stderr, code) = run(&[
            "route", &dsn, "-o", &output.to_string_lossy(), "--max-passes", "1",
        ]);
        assert_eq!(code, 0, "-o {name}:\n{stderr}");
        assert!(std::fs::metadata(&output).is_ok_and(|meta| meta.len() > 0));
    }
}

#[test]
fn a_session_under_a_dsn_name_exits_1() {
    let dir = scratch("session-as-dsn");
    let input = dir.join("board.dsn");
    std::fs::write(&input, b"(session previous)\n").unwrap();
    let (stdout, stderr, code) = run(&[
        "route", &input.to_string_lossy(), "-o", &dir.join("out.ses").to_string_lossy(),
    ]);
    assert_eq!(code, 1);
    assert!(stdout.is_empty(), "{stdout}");
    assert!(stderr.contains("not a board"), "{stderr}");
}

#[test]
fn set_reaches_the_run_and_the_dotted_spelling_is_a_usage_error() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = scratch("set");
    let dsn = small_dsn();
    let dsn = dsn.to_string_lossy();
    let via_costs = |manifest: &Path| settings_snapshot(manifest)["scoring"]["via_costs"].clone();

    for (name, spelling) in [("space", vec!["--set", "router.scoring.via_costs=77"]),
                             ("equals", vec!["--set=router.scoring.via_costs=77"])] {
        let manifest = dir.join(format!("{name}.json"));
        let mut argv = vec!["route", &dsn, "-o", "x.ses", "--max-passes", "1", "--result-json"];
        let manifest_text = manifest.to_string_lossy().into_owned();
        let ses = dir.join(format!("{name}.ses")).to_string_lossy().into_owned();
        argv[3] = &ses;
        argv.push(&manifest_text);
        argv.extend(spelling);
        let (_, stderr, code) = run(&argv);
        assert_eq!(code, 0, "{name}: {stderr}");
        assert_eq!(via_costs(&manifest), serde_json::json!(77), "{name}");
    }

    let (_, stderr, code) = run(&[
        "route", &dsn, "-o", &dir.join("d.ses").to_string_lossy(), "--router.scoring.via_costs=77",
    ]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stderr.contains("unexpected argument"), "{stderr}");

    let (_, stderr, code) = run(&[
        "route", &dsn, "-o", &dir.join("e.ses").to_string_lossy(), "--set", "scoring.via_costs=77",
    ]);
    assert_eq!(code, 1, "a --set without the router. prefix is refused: {stderr}");
    assert!(stderr.contains("router."), "{stderr}");
}

#[test]
fn max_passes_and_timeout_reach_the_settings() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = scratch("max-passes-timeout");
    let manifest = dir.join("m.json");
    let (_, stderr, code) = run(&[
        "route", &small_dsn().to_string_lossy(), "-o", &dir.join("out.ses").to_string_lossy(),
        "--max-passes", "3", "--timeout", "1:00:00", "--result-json", &manifest.to_string_lossy(),
    ]);
    assert_eq!(code, 0, "{stderr}");
    let snapshot = settings_snapshot(&manifest);
    assert_eq!(snapshot["max_passes"], serde_json::json!(3));
    assert_eq!(snapshot["job_timeout"], serde_json::json!("1:00:00"));
}

#[test]
fn a_settings_file_reaches_the_run_and_the_working_directory_is_not_read() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = scratch("settings-file");
    let json = dir.join("s.json");
    std::fs::write(&json, r#"{"router": {"scoring": {"via_costs": 77}}}"#).unwrap();
    let cwd = dir.join("cwd");
    std::fs::create_dir_all(&cwd).unwrap();
    std::fs::write(cwd.join("freerouting.json"), r#"{"router": {"scoring": {"via_costs": 77}}}"#).unwrap();
    let dsn = small_dsn();
    let dsn = dsn.to_string_lossy();
    let via_costs = |manifest: &Path| settings_snapshot(manifest)["scoring"]["via_costs"].clone();

    let b = dir.join("b.json");
    let (_, stderr, code) = run(&[
        "--settings", &json.to_string_lossy(), "route", &dsn, "-o",
        &dir.join("b.ses").to_string_lossy(), "--max-passes", "1", "--result-json", &b.to_string_lossy(),
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(via_costs(&b), serde_json::json!(77));

    let e = dir.join("e.json");
    let output = std::process::Command::new(PORT)
        .current_dir(&cwd)
        .args(["route", &dsn, "-o", &dir.join("e.ses").to_string_lossy(), "--max-passes", "1",
               "--result-json", &e.to_string_lossy()])
        .stdin(std::process::Stdio::null())
        .output()
        .expect("the port runs");
    assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(via_costs(&e), serde_json::json!(50), "no default settings file is read");

    let (_, stderr, code) = run(&[
        "--settings", "/nonexistent/s.json", "route", &dsn, "-o", &dir.join("f.ses").to_string_lossy(),
    ]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("/nonexistent/s.json"), "{stderr}");
}

#[test]
fn final_state_distinguishes_stage_limits_from_job_deadlines() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = scratch("stage-timeout");
    let dsn = small_dsn();
    let dsn = dsn.to_string_lossy();

    let manifest = dir.join("stage.json");
    let (_, stderr, code) = run(&[
        "route", &dsn, "-o", &dir.join("stage.ses").to_string_lossy(), "--max-passes", "1",
        "--set", "router.optimizer.timeout=0:00:00", "--result-json", &manifest.to_string_lossy(),
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(final_state(&manifest), "COMPLETED");

    let manifest = dir.join("job.json");
    let (_, stderr, code) = run(&[
        "route", &dsn, "-o", &dir.join("job.ses").to_string_lossy(), "--max-passes", "1",
        "--timeout", "0:00:00", "--result-json", &manifest.to_string_lossy(),
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(final_state(&manifest), "TIMED_OUT");
}

#[test]
fn an_invalid_input_writes_no_manifest() {
    let dir = scratch("final-state-invalid");
    let input = dir.join("board.dsn");
    std::fs::write(&input, b"(session previous)\n").unwrap();
    let manifest = dir.join("m.json");
    let (_, stderr, code) = run(&[
        "route", &input.to_string_lossy(), "-o", &dir.join("out.ses").to_string_lossy(),
        "--result-json", &manifest.to_string_lossy(),
    ]);
    assert_eq!(code, 1, "{stderr}");
    assert!(!manifest.exists());
}

#[test]
fn drc_exits_1_on_violations_and_0_on_a_clean_board() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = scratch("drc-exit");
    let out = dir.join("dirty.json");
    let (_, stderr, code) = run(&["drc", &drc_dsn().to_string_lossy(), "-o", &out.to_string_lossy()]);
    assert_eq!(code, 1, "{stderr}");
    assert_eq!(report(&out)["violations"].as_array().unwrap().len(), 8);

    let clean = dir.join("clean.json");
    let tutorial = parity::example("tutorial_board/tutorial_board.dsn");
    let (_, stderr, code) = run(&["drc", &tutorial.to_string_lossy(), "-o", &clean.to_string_lossy()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(report(&clean)["violations"].as_array().unwrap().is_empty());
}

#[test]
fn drc_only_warns_about_a_missing_rules_or_session_file() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = scratch("drc-missing-aux");
    let dsn = drc_dsn();
    for (flag, value) in [("--rules", "nosuch.rules"), ("--ses", "nosuch.ses")] {
        let out = dir.join(format!("{value}.json"));
        let (_, stderr, code) = run(&[
            "drc", &dsn.to_string_lossy(), flag, &dir.join(value).to_string_lossy(), "-o", &out.to_string_lossy(),
        ]);
        assert_eq!(code, 1, "the report is written and the board's violations set the code:\n{stderr}");
        assert!(stderr.contains("not read"), "{stderr}");
        assert!(out.is_file());
    }
}

#[test]
fn drc_exits_1_when_the_input_is_unreadable_or_not_a_board_or_unwritable() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = scratch("drc-refusals");
    let (_, stderr, code) = run(&["drc", "/nonexistent/board.dsn", "-o", &dir.join("a.json").to_string_lossy()]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("Couldn't load the input file"), "{stderr}");

    let session = dir.join("session.dsn");
    std::fs::write(&session, b"(session previous)\n").unwrap();
    let (_, stderr, code) = run(&["drc", &session.to_string_lossy(), "-o", &dir.join("b.json").to_string_lossy()]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("not a board"), "{stderr}");

    let out = dir.join("nodir").join("r.json");
    let (_, stderr, code) = run(&["drc", &drc_dsn().to_string_lossy(), "-o", &out.to_string_lossy()]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("Couldn't save the DRC report"), "{stderr}");
    assert!(!out.exists());
}
```

Delete: `de_a_ses_exits_1_instead_of_hanging` (replaced by `a_session_under_a_dsn_name_exits_1`), `the_generic_override_is_set_on_native_and_dotted_on_legacy` (replaced), `a_settings_file_reaches_the_run` (replaced), `an_invalid_input_writes_no_manifest_because_java_never_reaches_the_writer` (replaced), `drc_exits_0_when_the_rules_file_is_missing`, `drc_exits_1_when_the_input_is_unreadable`, `drc_exits_1_when_the_board_will_not_load`, `drc_exits_1_when_the_report_cannot_be_written` (all three folded into the last test above), `the_cli_refuses_an_unreadable_router_budget_with_exit_2`.

In `climb_one`: delete the `expected_log`/`actual_log` block, and change the manifest run to push `"--result-json".to_string()` and `manifest.display().to_string()` as two tokens instead of `--router.result_json=`. In `regolden_one`: delete the `route.log` write and make the same manifest-argv change. Rename `the_ci_stems_match_the_jars_reference` → `the_ci_stems_match_the_reference` and its slow sibling likewise, and the `SKIP` message to name `FR_REGOLDEN` instead of `gen-cli-reference.sh`.

`parity::example` exists (`pub fn example(name: &str) -> PathBuf`); if its argument shape differs, use `parity::java_dir().join("examples/tutorial_board/tutorial_board.dsn")`.

- [ ] **Step 4: Run the CLI end-to-end suite**

Run: `cargo test -p freerouting --test cli_e2e`
Expected: all pass, `the_ci_stems_match_the_reference` included — the SES bytes and normalised manifests of the four CI stems are unchanged. If a stem's SES differs, stop: the loader changed routing input (rules discovery, DSN block, budget) and Task 2/3 must be corrected, never the golden.

- [ ] **Step 5: Run the slow lane once**

Run: `FR_SLOW_PARITY=1 cargo test -p freerouting --release --test cli_e2e the_slow_stems_match_the_reference`
Expected: pass (a few minutes).

- [ ] **Step 6: Commit**

```bash
git add -A tests/reference tests/parity/src/lib.rs crates/freerouting/tests/cli_e2e.rs
git commit -m "test(cli): the end-to-end suite and the committed argv on the native form"
```

---

### Task 8: Version stamps

**Files:**
- Modify: `crates/fr-core/src/lib.rs`, `crates/fr-core/src/manifest.rs`, `crates/fr-core/tests/manifest.rs`, `crates/fr-drc/tests/common/mod.rs`, `crates/fr-drc/tests/data/README.md`
- Test: `crates/fr-core/tests/manifest.rs`

- [ ] **Step 1: Write the failing test**

In `crates/fr-core/tests/manifest.rs` replace `app_version_is_the_parity_version` with:

```rust
#[test]
fn app_version_is_the_crate_version() {
    let manifest = from_job(&fresh_job(), None, false, 1, None);
    assert_eq!(
        manifest.app_version.as_deref(),
        Some(fr_core::SERVER_VERSION)
    );
    assert_eq!(fr_core::SERVER_VERSION, env!("CARGO_PKG_VERSION"));
}
```

and change the `"\"app_version\": \"2.3.1-SNAPSHOT\""` assertion a few lines above it to `text.contains(&format!("\"app_version\": \"{}\"", fr_core::SERVER_VERSION))`.

Run: `cargo test -p fr-core --test manifest app_version` — expected: FAIL.

- [ ] **Step 2: Implement**

In `crates/fr-core/src/lib.rs` delete the three `PARITY_*` constants. In `crates/fr-core/src/manifest.rs` replace `use crate::PARITY_VERSION;` with `use crate::SERVER_VERSION;` and `PARITY_VERSION.to_string()` with `SERVER_VERSION.to_string()`. In `crates/fr-drc/tests/common/mod.rs` replace the constant with:

```rust
/// The version string every committed DRC transcript was written with.
#[allow(dead_code)]
pub const JAR_VERSION: &str = "2.3.1-SNAPSHOT";
```

and remove `fr-core` from `[dev-dependencies]` in `crates/fr-drc/Cargo.toml` if nothing else in `crates/fr-drc/tests` uses it (`grep -rn fr_core crates/fr-drc/tests`). In `crates/fr-drc/tests/data/README.md` delete the two sentences that say a rebuilt jar needs `JAR_VERSION` changed.

- [ ] **Step 3: Run the tests**

Run: `cargo test -p fr-core --test manifest && cargo test -p fr-drc && cargo test -p freerouting --test cli_e2e the_ci_stems_match_the_reference`
Expected: pass. The manifest goldens still compare equal because Task 7 removed `app_version` from `normalize_manifest`.

- [ ] **Step 4: Commit**

```bash
git add crates/fr-core crates/fr-drc
git commit -m "chore(core): stamp the crate version, not the parity version"
```

---

### Task 9: Differential drivers, generator scripts, docs

**Files:**
- Delete: `scripts/differential/rust/src/bin/{p8t1,p8t2,p8t3,p8t5,p8t7}.rs`, `scripts/differential/java/{P8T2,P8T3,P8T5}.java`, `scripts/gen-cli-reference.sh`, `docs/cli-legacy-flags.md`
- Modify: `scripts/differential/rust/Cargo.toml`, `scripts/gen-drc-reference.sh`, `scripts/gen-batch-reference.sh`, `scripts/quality-ab.sh`

- [ ] **Step 1: Delete the drivers and their manifest entries**

```bash
git rm -q scripts/differential/rust/src/bin/p8t1.rs scripts/differential/rust/src/bin/p8t2.rs \
  scripts/differential/rust/src/bin/p8t3.rs scripts/differential/rust/src/bin/p8t5.rs \
  scripts/differential/rust/src/bin/p8t7.rs scripts/differential/java/P8T2.java \
  scripts/differential/java/P8T3.java scripts/differential/java/P8T5.java \
  scripts/gen-cli-reference.sh docs/cli-legacy-flags.md
```

In `scripts/differential/rust/Cargo.toml` delete the five `[[bin]]` blocks named `p8t2`, `p8t5`, `p8t1`, `p8t3`, `p8t7` (each with its comment), and delete the `freerouting = { path = … }` dependency and its comment if no remaining bin imports `freerouting::` (`grep -ln 'freerouting::' scripts/differential/rust/src/bin/*.rs`). Keep the `parity` dependency (`refwriter` and others may use it; check with `grep -ln 'parity::' scripts/differential/rust/src/bin/*.rs` and drop it only if nothing matches).

Run: `(cd scripts/differential/rust && cargo check --bins)` — expected: builds. Also delete any `p8t1`/`p8t2`/`p8t3`/`p8t5`/`p8t7`/`P8T2`/`P8T3`/`P8T5` rows from `scripts/differential/matrix/` and from the differential README if one exists (`ls scripts/differential`).

- [ ] **Step 2: `gen-drc-reference.sh`**

The port lane builds its argv at lines 156–159 (`ARGS=(-de …) … ARGS+=(-drc "$out")`). Make the argv depend on the lane: keep the legacy `ARGS` for the jar and build a second array for the port:

```bash
  ARGS=(-de "$JAVA_DIR/$dsn")
  [[ -n "$ses" ]] && ARGS+=("$JAVA_DIR/$ses")
  [[ -n "$rules" ]] && ARGS+=(-dr "$JAVA_DIR/$rules")
  ARGS+=(-drc "$out")
  PORT_ARGS=(drc "$JAVA_DIR/$dsn")
  [[ -n "$ses" ]] && PORT_ARGS+=(--ses "$JAVA_DIR/$ses")
  [[ -n "$rules" ]] && PORT_ARGS+=(--rules "$JAVA_DIR/$rules")
  PORT_ARGS+=(-o "$out")
```

and in `run_drc` use `"$PORT_BIN" "${PORT_ARGS[@]}"` in the port branch (the function receives `"$@"` today; pass `"${PORT_ARGS[@]}"` from the call site when `LANE == port`, or read the global). Because `drc` now exits 1 on violations, append `|| true` to the port invocation so a report with violations does not abort the generator, and let the `[[ -s "$out" ]]` check that follows decide. Adjust the comment above it to one line: `# The port's native form; the jar keeps its own.`

- [ ] **Step 3: `gen-batch-reference.sh`**

Replace the port branch of `run_bare_program` (lines 325–330) with:

```bash
    "${TIMEOUT[@]}" "$PORT_BIN" \
        route "$dsn" -o "$ses" --max-passes "$max_passes" \
        --set "router.fanout.enabled=$fanout" \
        --set "router.optimizer.enabled=$optimizer" \
        > "$log" 2>&1 < /dev/null
```

- [ ] **Step 4: `quality-ab.sh`**

Four edits:

1. `run_referee` (line ~585): `"${TIMEOUT[@]}" "$PORT_BIN" "$@" -drc "$report"` → `"${TIMEOUT[@]}" "$PORT_BIN" "$@" -o "$report"`.
2. `referee_argv_for`: build the native form —
   ```bash
   REFEREE_ARGV=(drc "$JAVA_DIR/$board")
   if [[ "$family" == drc ]]; then
     [[ -n "$sesfile" ]] && REFEREE_ARGV+=(--ses "$JAVA_DIR/$sesfile")
     [[ -n "$rules" ]] && REFEREE_ARGV+=(--rules "$JAVA_DIR/$rules")
   else
     REFEREE_ARGV+=(--ses "$SCRATCH/$family-$stem.ses")
   fi
   ```
3. The quality lane (line ~747): `local -a route_argv=(route "$JAVA_DIR/$board" -o "$ses" "${QUALITY_LANE_ARGS[@]}" ${extra+"${extra[@]}"} --result-json "$manifest")`, and replace `FR_ROUTER_BUDGET="$QUALITY_LANE_BUDGET" "${TIMEOUT[@]}" "$PORT_BIN"` with `"${TIMEOUT[@]}" "$PORT_BIN"`. Define, where `QUALITY_LANE_BUDGET` is defined, `QUALITY_LANE_ARGS=(--set router.opt_changed_area_ms=0 --set router.fanout.max_milliseconds_per_pin=2147483647)` and delete `QUALITY_LANE_BUDGET`. Rewrite the header comment paragraphs that describe `FR_ROUTER_BUDGET` (lines ~49, ~230, ~480) to say the lane passes those two `--set` overrides.
4. The time lane (line ~822–829): the DRC branch passes `"${REFEREE_ARGV[@]}" -o "$SCRATCH/…json"`; the route branch becomes `route "$JAVA_DIR/$board" -o "$SCRATCH/$family-$stem.t$i.ses" ${time_extra+"${time_extra[@]}"}`.

The fixture `extra_args` columns the script reads come from `tests/reference/cli-fixtures.txt`, rewritten in Task 7 to the native spelling, so `${extra[@]}` already carries `--max-passes … --set …`. The DRC lane's `-drc` in the referee row-parser comment (line ~1191) becomes `drc <board> --ses <ses> -o <report>`.

Run: `bash -n scripts/quality-ab.sh scripts/gen-drc-reference.sh scripts/gen-batch-reference.sh` — expected: no syntax errors. Then `shellcheck` on the three if it is installed.

- [ ] **Step 5: Commit**

```bash
git add -A scripts docs/cli-legacy-flags.md
git commit -m "chore(scripts): drop the jar-argv drivers and the CLI reference generator; native form in the scripts"
```

---

### Task 10: The benchmark runner

**Files:**
- Modify: `benchmark/bench/candidates.py`, `benchmark/tests/test_candidates.py`, `benchmark/README.md` (the paragraph that says candidates are invoked with legacy flags)

- [ ] **Step 1: Write the failing test**

Replace `test_argv_is_identical_shape_for_both_kinds` in `benchmark/tests/test_candidates.py` with:

```python
def test_argv_is_legacy_for_java_and_native_for_rust(toml_path):
    cands = load_candidates(toml_path)
    common = dict(in_dsn=Path("a.dsn"), out_ses=Path("a.ses"), result_json=Path("r.json"),
                  max_passes=100, timeout_s=300, threads=1, seed=2)
    j = cands["java-x"].argv(**common)
    assert j[3:] == ["-de", "a.dsn", "-do", "a.ses", "-mp", "100",
                     "--router.job_timeout=00:05:00", "--router.max_threads=1",
                     "--router.result_json=r.json", "--gui.enabled=false",
                     "--api_server.enabled=false", "--mcp_server.enabled=false"]
    r = cands["rs-x"].argv(**common)
    assert r[1:] == ["route", "a.dsn", "-o", "a.ses", "--max-passes", "100",
                     "--timeout", "00:05:00", "--result-json", "r.json",
                     "--set", "router.seed=2"]


def test_rust_extra_args_are_rewritten_from_the_dotted_spelling(toml_path):
    cands = load_candidates(toml_path)
    assert cands["rs-x"].extra_args == ["--router.seed={seed}"]
```

Run: `cd benchmark && uv run pytest tests/test_candidates.py -q` — expected: FAIL.

- [ ] **Step 2: Implement**

In `benchmark/bench/candidates.py` replace the module docstring with `"""Candidate routers: black-box commands, invoked in each kind's own command-line form."""` and replace `argv` with:

```python
    def argv(self, *, in_dsn: Path, out_ses: Path, result_json: Path,
             max_passes: int, timeout_s: int, threads: int, seed: int) -> list[str]:
        if self.kind == "rust":
            return self._native_argv(in_dsn=in_dsn, out_ses=out_ses, result_json=result_json,
                                     max_passes=max_passes, timeout_s=timeout_s, seed=seed)
        args = [
            *self.exec,
            "-de", str(in_dsn),
            "-do", str(out_ses),
            "-mp", str(max_passes),
            f"--router.job_timeout={self.hms(timeout_s)}",
            f"--router.max_threads={threads}",
            f"--router.result_json={result_json}",
            "--gui.enabled=false",
            "--api_server.enabled=false",
            "--mcp_server.enabled=false",
        ]
        args += [a.format(seed=seed) for a in self.extra_args]
        return args

    def _native_argv(self, *, in_dsn: Path, out_ses: Path, result_json: Path,
                     max_passes: int, timeout_s: int, seed: int) -> list[str]:
        args = [
            *self.exec,
            "route", str(in_dsn),
            "-o", str(out_ses),
            "--max-passes", str(max_passes),
            "--timeout", self.hms(timeout_s),
            "--result-json", str(result_json),
        ]
        for extra in self.extra_args:
            extra = extra.format(seed=seed)
            if extra.startswith("--router."):
                args += ["--set", extra[2:]]
            else:
                args.append(extra)
        return args
```

`threads` is accepted and ignored for the Rust candidate: the router is single-threaded.

- [ ] **Step 3: Run the benchmark tests**

Run: `cd benchmark && uv run pytest -q`
Expected: pass.

- [ ] **Step 4: Update the README paragraph**

In `benchmark/README.md`, find the sentence describing how candidates are invoked with `-de/-do` and change it to say that `java` candidates take the classic flags and `rust` candidates take `route <dsn> -o <ses> --max-passes N --timeout T --result-json F`, with `extra_args` of the form `--router.x=y` rewritten to `--set router.x=y`.

- [ ] **Step 5: Commit**

```bash
git add benchmark/bench/candidates.py benchmark/tests/test_candidates.py benchmark/README.md
git commit -m "feat(benchmark): per-kind argv — the Rust candidate uses the native command line"
```

---

### Task 11: READMEs

**Files:**
- Modify: `crates/freerouting/README.md`, `crates/fr-core/README.md`

- [ ] **Step 1: `crates/freerouting/README.md`**

Rewrite the file to describe only the native form. Sections: the four subcommands and their flags (copy the table from the spec's "The command line" section); the exit ladder (0 / 1 / 2, `drc` exits 1 on violations); settings precedence (defaults, `--settings`, the design's block, the rules file, `--max-passes`/`--timeout`/`--set`, the MCP payload); the MCP server and its four tools (keep the existing "The MCP server" and "The four tools" sections, minus the comparison rows against another program and minus the `route -de board.json` aside); logging (stderr, no timestamps, `-v`/`--log-level`); tests (`cli_e2e.rs`, `mcp_stdio.rs`, `ops_*.rs`, the `ci`/`slow` lanes, `FR_REGOLDEN=<label> cargo test -p freerouting --test cli_e2e` to re-cut goldens); the remaining generators (`gen-drc-reference.sh`, `gen-batch-reference.sh`, `gen-router-reference.sh`, `gen-reference.sh`). Delete every mention of the legacy form, `--kicad-json`, `FR_ROUTER_BUDGET`, `MESSAGE_MAP`, `gen-cli-reference.sh`, `docs/cli-legacy-flags.md` and the `p8t*` drivers.

- [ ] **Step 2: `crates/fr-core/README.md`**

In the "Versions" section replace the four-line block with `SERVER_VERSION = env!("CARGO_PKG_VERSION")` and the paragraph with: the version written into DRC reports and manifests and the MCP `serverInfo` is the crate's own. In "The job model" delete the sentence about `--max-items`'s help text if it references the legacy spelling; in "`CancelToken` and `RouterStop`" change `--set router.max_items=N` prose to say the flag lives on `route`.

- [ ] **Step 3: Check for leftovers**

Run: `grep -rn -- '-de \|--kicad-json\|FR_ROUTER_BUDGET\|MESSAGE_MAP\|gen-cli-reference\|cli-legacy-flags\|legacy' crates/*/README.md crates/freerouting/src docs/routing-visualizer.md`
Expected: nothing.

- [ ] **Step 4: Commit**

```bash
git add crates/freerouting/README.md crates/fr-core/README.md
git commit -m "docs(cli): READMEs for the native command line"
```

---

### Task 12: Whole-workspace verification

**Files:** none new.

- [ ] **Step 1: Format, lint, test**

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace || cargo test --workspace
(cd scripts/differential/rust && cargo check --bins)
(cd benchmark && uv run pytest -q)
```

Expected: every command exits 0. Fix any clippy finding in the files this plan touched (unused imports from the rewrites are the likely ones).

- [ ] **Step 2: Grep for the removed surface**

```bash
grep -rn 'legacy::\|settings_argv\|EnvironmentVariablesSource\|FR_ROUTER_BUDGET\|PARITY_VERSION\|normalize_log\|run_jar' crates tests/parity/src scripts/*.sh benchmark/bench
```

Expected: nothing except `EnvironmentVariablesSource` inside `crates/fr-settings` (the source type stays in that crate; only the CLI stopped feeding it).

- [ ] **Step 3: Confirm the goldens did not move**

```bash
git status --short tests/reference | grep -v 'argv.txt\|route.log\|cli-fixtures.txt' || echo "no golden moved"
```

Expected: `no golden moved`.

- [ ] **Step 4: Commit any fix-ups**

```bash
git add -A && git commit -m "chore(cli): clippy and fmt after the CLI simplification" || true
```

---

## Self-review

**Spec coverage.** Legacy form dropped: Task 5. Settings sources: Task 1 (env removed, file + set + sparse), `FR_ROUTER_BUDGET`: Tasks 5, 7, 9. Dead flags and `--kicad-json`: Task 5. Exit codes and `drc` violations: Tasks 5, 7. Plain logs, `MESSAGE_MAP`: Tasks 5, 7. Version stamps: Task 8. One loader and request types for CLI and MCP: Tasks 1–6. Argv files, parity helper, `cli_e2e`: Task 7. Differential drivers, generator scripts, `quality-ab.sh`, `docs/cli-legacy-flags.md`: Task 9. Benchmark runner: Task 10. READMEs: Task 11. Goldens unchanged: Tasks 7 and 12.

**Type consistency.** `SettingsOverrides { settings_file, set, sparse }` is used identically in Tasks 1, 2, 5, 6. `LoadRequest::for_board` plus field assignment is the construction pattern everywhere. `OutputTarget::{File, Session}` and `OutputFormat::from_path` in Tasks 3, 5. `route(RouteRequest)` by value, `drc(&DrcRequest)` and `info(&InfoRequest)` by reference, in Tasks 3–6. `ExitCode` lives in `freerouting::ExitCode` from Task 5 on. `State::with_overrides` and `State.overrides` in Tasks 5 and 6. `mcp::stdio::run(SettingsOverrides)` in Tasks 5 and 6.

**Known judgement calls the executor may hit.** `apply_new_values_from` visibility (Task 1 says what to do). `BoardFileDetails::filename` visibility (Task 3 says what to do). The DRC quality score's settings source (Task 4 names the oracle). `parity::example`'s signature (Task 7 gives the fallback).
