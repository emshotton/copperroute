//! Rust half of the `p7t9` differential pair — `scripts/differential/java/P7T9.java`.
//!
//! Plan 7 Task 10, whole-board level: `AutorouteBatchLoop::run` (`AutorouteBatchLoop.java:37-588`)
//! — the pass loop, its best-board policy and its two stagnation detectors — over a real DSN
//! board. This is the whole `-dr`-equivalent routing stage with fanout and the optimizer off.
//!
//! Usage: `p7t9 <dsn> [maxPasses] [mode]`. See `P7T9.java`'s class comment for the two halves, for
//! why the `[real]` half is what makes the `[transcript]` half evidence, for why the driver calls
//! `runBatchLoop()` rather than `RoutingPipeline.run()`, and for the budget note. The shared board
//! and settings ladder is [`p7t_common`].
//!
//! # The board comparison
//!
//! Java compares its two boards with `BasicBoard.getHash()`, an MD5 over `serialize(true)`; this
//! side compares with [`fr_board::Board::structural_hash`], a `u64` over a Rust-native input. The
//! two are **not** comparable by value and neither is printed — what crosses the diff is the
//! **decision** `equalsTranscript=<bool>`, which is controller ruling AH's rule and what Task 3
//! audited the port's hash against.
//!
//! # `FINISH-AUTOROUTE`
//!
//! Java's line calls `RoutingBoard.finishAutoroute()` — the one call `RoutingPipeline.java:106`
//! makes after `runBatchLoop()` — and prints whether the board hash moved. The port's
//! `Board::finish_autoroute` is empty (the autoroute engine is not a `Board` field here, plan-6
//! ruling 3), so this side prints the constant `moved=false`; a Java-side `true` would be a real
//! finding about the pipeline seam Task 15 builds on.

use std::io::{BufWriter, Write};

use fr_board::prelude::*;
use fr_dsn::{java_double_to_string, java_float_to_string};
use fr_router::pipeline::{
    optimizer_route_improved, prepare_board, run_pipeline, AutorouteBatchLoop, BatchLoopResult,
    BatchOptimizer, ItemRouteResult, NamedAlgorithmType, NoopProgressSink, PipelineResult,
    ProgressSink, ReadSortedRouteItems, RouterBudget, RouterStop, RoutingEvent, TaskState,
};
use fr_router::score::BoardStatistics;
use fr_settings::sources::{CliSettings, DsnFileSettings, EnvironmentVariablesSource};
use fr_settings::{
    resolve_headless, HostEnvironment, RouterSettings, SettingsInputs, SettingsSource,
};

#[path = "../p7t_common.rs"]
mod p7t_common;

fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.is_empty() {
        eprintln!(
            "usage: p7t9 <dsn> [maxPasses] [mode] [optPasses|all] [optItems|all] \
             [--fanout on|off] [--optimizer on|off] [--ses <path>] [--passes <path>]"
        );
        std::process::exit(2);
    }
    // Plan 7 Task 16's trailing flags, parsed out first so the five positionals keep the meanings
    // Tasks 10/14/15 gave them and no existing invocation changes. `P7T9.main`'s switch.
    let mut args: Vec<String> = Vec::new();
    let mut ses_path: Option<std::path::PathBuf> = None;
    let mut passes_path: Option<std::path::PathBuf> = None;
    let mut fanout_flag: Option<bool> = None;
    let mut optimizer_flag: Option<bool> = None;
    let mut it = raw.into_iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--ses" => ses_path = Some(it.next().expect("--ses needs a path").into()),
            "--passes" => passes_path = Some(it.next().expect("--passes needs a path").into()),
            "--fanout" => fanout_flag = Some(on_off(&it.next().expect("--fanout needs on|off"))),
            "--optimizer" => {
                optimizer_flag = Some(on_off(&it.next().expect("--optimizer needs on|off")));
            }
            _ => args.push(arg),
        }
    }
    let dsn = std::fs::canonicalize(&args[0])
        .unwrap_or_else(|e| panic!("cannot resolve {}: {e}", args[0]));
    let max_passes: i32 = args
        .get(1)
        .filter(|a| !a.is_empty())
        .map_or(1, |a| a.parse().expect("maxPasses"));
    let mode: &str = args
        .get(2)
        .filter(|a| !a.is_empty())
        .map_or("router-only", String::as_str);
    if !matches!(
        mode,
        "router-only"
            | "router+fanout"
            | "optimizer"
            | "optimizer+fanout"
            | "optimizer-shared"
            | "full"
            | "batch"
            | "batch-router"
    ) {
        eprintln!(
            "p7t9: mode must be 'router-only', 'router+fanout', 'optimizer', 'optimizer+fanout', \
             'optimizer-shared', 'full', 'batch' or 'batch-router', not: {mode}"
        );
        std::process::exit(2);
    }
    // `None` is Java's `null`, i.e. "no limit" for both `optimizer.maxPasses` (`:167-168`) and
    // `optimizer.maxItems` (`:169-170`, `:318-320`), and `all` is how the command line spells it.
    let opt_passes: Option<i32> = args
        .get(3)
        .filter(|a| !a.is_empty())
        .map_or(Some(2), |a| boxed_limit(a));
    let opt_items: Option<i32> = args
        .get(4)
        .filter(|a| !a.is_empty())
        .and_then(|a| boxed_limit(a));

    let stdout = std::io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    let (jar, bytes, mtime) = p7t_common::jar_identity();
    writeln!(
        out,
        "HEADER jar={jar} bytes={bytes} mtime={mtime} fixture={} maxPasses={max_passes} \
         mode={mode}{}",
        dsn.file_name().expect("a file name").to_string_lossy(),
        if is_batch_mode(mode) {
            format!(
                " fanout={} optimizer={}",
                on_off_name(fanout_flag.unwrap_or(true)),
                on_off_name(optimizer_flag.unwrap_or(true))
            )
        } else if is_optimizer_mode(mode) || mode == "full" {
            format!(
                " optPasses={} optItems={}",
                limit_name(opt_passes),
                limit_name(opt_items)
            )
        } else {
            String::new()
        },
    )
    .expect("write");
    if let Ok(exe) = std::env::current_exe() {
        eprintln!("rust-binary {}", exe.display());
    }

    if is_batch_mode(mode) {
        run_batch_mode(
            &mut out,
            &dsn,
            max_passes,
            mode,
            fanout_flag.unwrap_or(true),
            optimizer_flag.unwrap_or(true),
            ses_path.as_deref(),
            passes_path.as_deref(),
        );
        out.flush().expect("flush");
        return;
    }

    if is_optimizer_mode(mode) {
        run_optimizer_mode(&mut out, &dsn, max_passes, mode, opt_passes, opt_items);
        out.flush().expect("flush");
        return;
    }

    if mode == "full" {
        run_full_mode(&mut out, &dsn, max_passes, opt_passes, opt_items);
        out.flush().expect("flush");
        return;
    }

    // ---- half one: the transcription ---------------------------------------------------------
    let mut board = p7t_common::load_board(&dsn);
    let settings = build_settings(&board, max_passes, mode);
    writeln!(out, "[transcript]").expect("write");
    let transcript_returned = transcribe_run(&mut out, &mut board, &settings);
    writeln!(
        out,
        "TRANSCRIPT returned={transcript_returned} {}",
        p7t_common::board_shape(&mut board)
    )
    .expect("write");
    let transcript_hash = board.structural_hash();

    // ---- half two: the real method -------------------------------------------------------------
    let mut real_board = p7t_common::load_board(&dsn);
    let real_settings = build_settings(&real_board, max_passes, mode);
    let real_stop = RouterStop::new();
    let mut real_sink = NoopProgressSink;
    writeln!(out, "[real]").expect("write");
    let real_result: BatchLoopResult = AutorouteBatchLoop::run(
        &mut real_board,
        &real_settings,
        &real_stop,
        RouterBudget::disabled(),
        &mut real_sink,
    )
    .expect("the corpus stems all have a routable signal layer");
    let real_hash = real_board.structural_hash();
    writeln!(
        out,
        "REAL returned={} {} equalsTranscript={}",
        real_result.continue_routing,
        p7t_common::board_shape(&mut real_board),
        real_hash == transcript_hash
    )
    .expect("write");
    // `RoutingBoard.finishAutoroute()` — see the module doc. There is nothing to call on this
    // side: plan-6 ruling 3 keeps the autoroute engine out of `Board` entirely, so the port has no
    // field for `finishAutoroute` to null out and `Board`'s own hook is an empty private stub
    // (`crates/fr-board/src/board/snapshot.rs`). The constant `false` is the port's answer, and a
    // Java-side `true` would be a real finding about the seam Task 15 builds on.
    let _ = real_hash;
    writeln!(out, "FINISH-AUTOROUTE moved=false").expect("write");

    // ---- the final board, in `P6T15aProbe`'s polyline format ------------------------------------
    writeln!(out, "[board]").expect("write");
    p7t_common::dump_board(&mut out, &board);
    out.flush().expect("flush");
}

// =================================================================================================
// Mode `full` — `run_pipeline`, Plan 7 Task 15
// =================================================================================================

/// Mode `full`: the actual [`run_pipeline`] — both stages, the fanout-only settings-clone
/// contract (unreachable from this driver's settings, which always sets `run_router = true`), the
/// one `finish_autoroute` no-op site and the optimizer-stage skip — driven directly, not through
/// a hand-written transcript. See `P7T9.runFullMode`'s doc for why: `run_pipeline` is short and
/// delegates to [`AutorouteBatchLoop::run`]/`BatchOptimizer::run_batch_loop`, both already pinned
/// end to end by the other four modes.
fn run_full_mode<W: Write>(
    out: &mut W,
    dsn: &std::path::Path,
    max_passes: i32,
    opt_passes: Option<i32>,
    opt_items: Option<i32>,
) {
    let mut board = p7t_common::load_board(dsn);
    let mut settings = build_settings(&board, max_passes, "router-only");
    settings.set_run_optimizer(true);
    apply_optimizer_limits(&mut settings, opt_passes, opt_items);

    let stop = RouterStop::new();
    let mut sink = Recorder::default();

    writeln!(out, "[pipeline]").expect("write");
    let result: PipelineResult = run_pipeline(
        &mut board,
        &settings,
        &stop,
        RouterBudget::disabled(),
        &mut sink,
    )
    .expect("the corpus stems all have a routable signal layer");

    for (algorithm, state) in sink.task_states() {
        writeln!(
            out,
            "EVENT algorithm={} state={}",
            java_algorithm_type(algorithm),
            java_task_state(state)
        )
        .expect("write");
    }

    writeln!(
        out,
        "RESULT passesRun={} optimizerPresent={} fanoutTimedOut={} optimizerTimedOut={} {}",
        result.router_passes_completed,
        result.optimizer_state.is_some(),
        result
            .fanout
            .as_ref()
            .is_some_and(|fanout| fanout.is_timed_out),
        // `PipelineResult` carries no separate per-stage optimizer-timeout bit — `timed_out`
        // folds it (and the fanout/router ones) into one flag; see `pipeline/run.rs`'s doc. This
        // line reads the same information Java's driver reads off `pipeline.getOptimizer().
        // isTimedOut()`, which on the port's side is exactly `result.timed_out` once the router
        // and fanout deadlines are ruled out — both are disabled on every corpus run here
        // (`RouterBudget::disabled()`, no `settings.optimizer.timeoutString`), so `timed_out`
        // alone already answers the optimizer's own case.
        result.timed_out,
        p7t_common::board_shape(&mut board)
    )
    .expect("write");

    writeln!(out, "[board]").expect("write");
    p7t_common::dump_board(out, &board);
}

/// Records every [`RoutingEvent::TaskStateChanged`], tagged by algorithm — the driver's own
/// `ProgressSink`, mirroring `P7T9.runFullMode`'s `TaskStateChangedEventListener`.
#[derive(Default)]
struct Recorder {
    events: Vec<RoutingEvent>,
}

impl ProgressSink for Recorder {
    fn on_event(&mut self, event: &RoutingEvent) {
        self.events.push(event.clone());
    }
}

impl Recorder {
    fn task_states(&self) -> Vec<(NamedAlgorithmType, TaskState)> {
        self.events
            .iter()
            .filter_map(|e| match e {
                RoutingEvent::TaskStateChanged { algorithm, state } => Some((*algorithm, *state)),
                _ => None,
            })
            .collect()
    }
}

/// Java's `NamedAlgorithmType.toString()`, i.e. the enum constant's own name.
fn java_algorithm_type(algorithm: NamedAlgorithmType) -> &'static str {
    match algorithm {
        NamedAlgorithmType::Router => "ROUTER",
        NamedAlgorithmType::Optimizer => "OPTIMIZER",
    }
}

/// `P7T9.buildSettings` — `p7t_common::build_settings` plus the driver's three knobs.
fn build_settings(board: &Board, max_passes: i32, mode: &str) -> RouterSettings {
    let mut settings = p7t_common::build_settings(board);
    settings.max_passes = Some(max_passes);
    // `RouterSettings.setFanoutEnabled(…)` (RouterSettings.java:583-591): the object must exist,
    // and `isFanoutEnabled` (`:578-580`) then answers the flag.
    let fanout = settings.fanout.get_or_insert_with(Default::default);
    fanout.enabled = Some(mode.ends_with("+fanout"));
    // Ruling AI, and the same knob `P7T9.buildSettings` writes: `fanoutPass:231-232` builds its
    // per-pin `TimeLimit` from this setting, so this is where the fanout stage's clock goes off.
    fanout.max_milliseconds_per_pin = Some(i64::from(i32::MAX));
    settings.set_run_router(true);
    // `RoutingPipeline`'s constructor (`RoutingPipeline.java:36`) builds an optimizer only when
    // this is set, which is what makes the optimizer modes a *pipeline* shape.
    settings.set_run_optimizer(is_optimizer_mode(mode));
    settings.save_intermediate_stages = Some(false);
    settings
}

/// `P7T9.isOptimizerMode` — the three modes that drive `BatchOptimizer::run_batch_loop`.
fn is_optimizer_mode(mode: &str) -> bool {
    mode.starts_with("optimizer")
}

// ================================================================================================
// Modes `batch` / `batch-router` — the jar's real `-de <dsn> -do <ses>` flow (Plan 7 Task 16)
// ================================================================================================

/// `P7T9.isBatchMode`.
fn is_batch_mode(mode: &str) -> bool {
    mode.starts_with("batch")
}

/// `P7T9.onOff` — the two words the fixture table's `fanout`/`optimizer` columns use.
fn on_off(arg: &str) -> bool {
    match arg {
        "on" => true,
        "off" => false,
        other => panic!("expected 'on' or 'off', not: {other}"),
    }
}

/// `P7T9.onOffName`, the inverse.
fn on_off_name(value: bool) -> &'static str {
    if value {
        "on"
    } else {
        "off"
    }
}

/// `P7T9.batchArgv` — the argv the bare jar is given for the same run.
fn batch_argv(
    dsn: &std::path::Path,
    max_passes: i32,
    fanout: bool,
    optimizer: bool,
) -> Vec<String> {
    let input = dsn.to_string_lossy().into_owned();
    let output = if let Some(stem) = input.strip_suffix(".dsn") {
        format!("{stem}.ses")
    } else {
        input.clone()
    };
    vec![
        "-de".to_string(),
        input,
        "-do".to_string(),
        output,
        "-mp".to_string(),
        max_passes.to_string(),
        format!("--router.fanout.enabled={fanout}"),
        format!("--router.optimizer.enabled={optimizer}"),
    ]
}

/// `P7T9.runBatchMode` — modes `batch` and `batch-router`, i.e. the whole-board run **as the CLI
/// runs it** (controller ruling AW).
///
/// Every other mode of this driver loads with `fr_dsn::read_board` and builds its settings from
/// `DefaultSettings` alone, matching `P7T2.loadBoard`/`buildSettings`. The jar's real
/// `-de x.dsn -do x.ses` flow does neither: it loads through `HeadlessBoardManager
/// .loadFromSpecctraDsn`, whose `applyRouterSettingsForLoadedBoard` (`:739-748`) **mutates the
/// board** through `applyCopperToEdgeClearanceOverride`/`applyHoleClearanceOverride` on 15 of the
/// 16 corpus boards (Task 15b), and it resolves settings through the two-merge ladder of
/// `Freerouting.java:125-146` + `RoutingJobScheduler.java:103-186`. This side is
/// [`resolve_headless`] — that ladder as one linear pass, already byte-pinned against the JVM by
/// Plan 4's `p4t1` — followed by [`prepare_board`], which is `:746-747`.
///
/// # Ordering
///
/// `resolve_headless` is called on the **pristine** board and `prepare_board` after it, where
/// Java's `:186` `applyBoardSpecificOptimizations` sees the already-mutated board. The two agree
/// because `applyBoardSpecificOptimizations` reads only the board's layer count and bounding box
/// (`RouterSettings.java:266-267`), and neither override touches either — they write the
/// clearance matrix and the outline's clearance class index. The `SETTINGS`/`BOARD-PREPARED`
/// lines below are what turn that argument into a measurement: both sides print them.
///
/// # The two modes
///
/// * `batch` runs the real [`run_pipeline`] and, with `--ses`, writes the result through
///   [`fr_dsn::ses_writer::write`] using the design name Java's `job.name` carries — the file
///   name without its extension. That is `tests/reference/<stem>/batch.ses`.
/// * `batch-router` runs the routing stage transcribed, on the same board and settings with the
///   optimizer off, so each completed pass prints its `PassRecord` tuple; `--passes` writes those
///   as JSON lines (`tests/reference/<stem>/batch.passes.jsonl`, ruling 1(a)'s rung).
#[allow(clippy::too_many_arguments)]
fn run_batch_mode<W: Write>(
    out: &mut W,
    dsn: &std::path::Path,
    max_passes: i32,
    mode: &str,
    fanout: bool,
    optimizer: bool,
    ses_path: Option<&std::path::Path>,
    passes_path: Option<&std::path::Path>,
) {
    let router_only = mode == "batch-router";
    let dsn_bytes = std::fs::read(dsn).unwrap_or_else(|e| panic!("cannot read {dsn:?}: {e}"));
    // `RoutingJob.setInputFromFile:432` stores the absolute path and `:457` derives `job.name`
    // from it — `BoardFileDetails.getFilename()` is the base name, `getFilenameWithoutExtension()`
    // that name without its last suffix.
    let file_name = dsn
        .file_name()
        .expect("a file name")
        .to_string_lossy()
        .into_owned();
    let design_name = file_name
        .rsplit_once('.')
        .map_or(file_name.clone(), |(stem, _)| stem.to_string());

    let argv = batch_argv(dsn, max_passes, fanout, optimizer && !router_only);
    writeln!(out, "ARGV {}", argv.join(" ")).expect("write");

    // The board, pristine — `DsnReader.readBoard` is what `loadFromSpecctraDsn:695` calls before
    // any of the manager's own work.
    let (mut board, transform) = p7t_common::load_board_with_transform(dsn);

    let dsn_source = DsnFileSettings::new(&dsn_bytes[..], &file_name);
    let env_map: std::collections::BTreeMap<String, String> = std::env::vars().collect();
    let env_source = EnvironmentVariablesSource::new(&env_map);
    let cli_source = CliSettings::new(&argv);
    // `JsonFileSettings` (priority 10) is not registered here. It was ported in Plan 8 Task 5
    // (scan ruling R7), but `resolve_headless` does not take it yet and this driver runs with no
    // `freerouting.json` in reach, so the tier is empty either way — which is the shape the Java
    // side is in too. ~~"has no port — spec §2 puts `freerouting.json` out of scope"~~
    // — and `SettingsInputs` has no slot for it. The generator records the file's presence and
    // the emptiness of its `router` scope in `batch.meta.txt`, so a machine where it stopped
    // being empty would be visible rather than silent.
    let inputs = SettingsInputs {
        json_file: None,
        dsn: dsn_source.get_settings(),
        cli_rules: None,
        // `RoutingJobScheduler.java:113-152` resolves `job.rules ?? -dr ?? adjacent
        // <design>.rules`; the generator passes no `-dr` and no corpus stem has an adjacent
        // `.rules`, so this is `None` and `:154-160`/`:173-184` are both skipped.
        scheduler_rules: None,
        env: env_source.get_settings(),
        cli: cli_source.get_settings(),
    };
    let settings = resolve_headless(&inputs, Some(&board), &HostEnvironment::detect());

    // `HeadlessBoardManager.java:746-747`, in Java's order.
    let prepared = prepare_board(&mut board, &settings);

    writeln!(
        out,
        "SETTINGS maxPasses={} maxItems={} runRouter={} runFanout={} runOptimizer={} \
         optMaxPasses={} optMaxItems={} fanoutMsPerPin={} tracePullTightAccuracy={}",
        java_boxed(settings.max_passes),
        java_boxed(settings.max_items),
        settings.get_run_router(),
        settings.is_fanout_enabled(),
        settings.get_run_optimizer(),
        java_boxed(settings.optimizer.as_ref().and_then(|o| o.max_passes)),
        java_boxed(settings.optimizer.as_ref().and_then(|o| o.max_items)),
        java_boxed(
            settings
                .fanout
                .as_ref()
                .and_then(|f| f.max_milliseconds_per_pin)
        ),
        java_boxed(settings.trace_pull_tight_accuracy)
    )
    .expect("write");
    let _ = prepared;
    writeln!(
        out,
        "BOARD-PREPARED classes={} outlineClass={} holeClearance={}",
        board.rules.clearance_matrix.get_class_count(),
        board
            .get_outline()
            .and_then(|id| board.items.get(&id))
            .map_or(-1, |item| i32::try_from(item.clearance_class())
                .unwrap_or(-1)),
        board.rules.get_hole_clearance()
    )
    .expect("write");

    if router_only {
        writeln!(out, "[transcript]").expect("write");
        let mut buffer: Vec<u8> = Vec::new();
        let returned = transcribe_run(&mut buffer, &mut board, &settings);
        out.write_all(&buffer).expect("write");
        writeln!(
            out,
            "TRANSCRIPT returned={returned} {}",
            p7t_common::board_shape(&mut board)
        )
        .expect("write");
        writeln!(out, "[board]").expect("write");
        p7t_common::dump_board(out, &board);
        if let Some(path) = passes_path {
            write_passes(path, &buffer);
        }
        if let Some(path) = ses_path {
            write_ses(path, &board, &transform, &design_name);
        }
        return;
    }

    let stop = RouterStop::new();
    let mut sink = Recorder::default();
    writeln!(out, "[pipeline]").expect("write");
    let result: PipelineResult = run_pipeline(
        &mut board,
        &settings,
        &stop,
        RouterBudget::disabled(),
        &mut sink,
    )
    .expect("the corpus stems all have a routable signal layer");

    for (algorithm, state) in sink.task_states() {
        writeln!(
            out,
            "EVENT algorithm={} state={}",
            java_algorithm_type(algorithm),
            java_task_state(state)
        )
        .expect("write");
    }
    writeln!(
        out,
        "RESULT passesRun={} optimizerPresent={} fanoutTimedOut={} optimizerTimedOut={} {}",
        result.router_passes_completed,
        result.optimizer_state.is_some(),
        result
            .fanout
            .as_ref()
            .is_some_and(|fanout| fanout.is_timed_out),
        result.timed_out,
        p7t_common::board_shape(&mut board)
    )
    .expect("write");
    writeln!(out, "[board]").expect("write");
    p7t_common::dump_board(out, &board);
    if let Some(path) = ses_path {
        write_ses(path, &board, &transform, &design_name);
    }
}

/// Java prints a boxed `Integer`/`Long` field with `String.valueOf`, i.e. `null` for absent.
fn java_boxed<T: std::fmt::Display>(value: Option<T>) -> String {
    value.map_or_else(|| "null".to_string(), |v| v.to_string())
}

/// `P7T9.writePasses` — one JSON object per completed pass, scraped from the transcription's own
/// `PASS` lines so the file and the transcript can never describe different passes.
fn write_passes(path: &std::path::Path, transcript: &[u8]) {
    let text = String::from_utf8_lossy(transcript);
    let mut rendered = String::new();
    for line in text.lines() {
        let Some(tuple) = line.strip_prefix("PASS ") else {
            continue;
        };
        rendered.push('{');
        for (index, field) in tuple.split(' ').enumerate() {
            let (key, value) = field.split_once('=').expect("a key=value field");
            if index > 0 {
                rendered.push(',');
            }
            rendered.push_str(&format!("\"{key}\":{value}"));
        }
        rendered.push_str("}\n");
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create the reference directory");
    }
    std::fs::write(path, rendered).expect("write the passes file");
}

/// `P7T9.writeSes` — `SesWriter.write(board, out, job.name)`, this side's `fr_dsn::ses_writer`.
fn write_ses(
    path: &std::path::Path,
    board: &Board,
    transform: &fr_dsn::CoordinateTransform,
    design_name: &str,
) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create the reference directory");
    }
    let mut sink = std::io::BufWriter::new(
        std::fs::File::create(path).unwrap_or_else(|e| panic!("cannot create {path:?}: {e}")),
    );
    fr_dsn::ses_writer::write(board, transform, &mut sink, design_name).expect("write the SES");
    sink.flush().expect("flush the SES");
}

/// `P7T9.boxedLimit` — `all` is Java's `null`.
fn boxed_limit(arg: &str) -> Option<i32> {
    if arg == "all" {
        None
    } else {
        Some(arg.parse().expect("an integer limit or `all`"))
    }
}

/// `P7T9.limitName`, the inverse.
fn limit_name(limit: Option<i32>) -> String {
    limit.map_or_else(|| "all".to_string(), |value| value.to_string())
}

// ------------------------------------------------------------------------------------------------
// run() — transcribed, AutorouteBatchLoop.java:37-588
// ------------------------------------------------------------------------------------------------

/// `P7T9.transcribeRun`: `run`'s body written out here against the port's own primitives, with one
/// line printed per decision the loop takes and one `PASS` tuple per completed pass.
///
/// The transcription is deliberately *not* a call to [`AutorouteBatchLoop::run`] — that is the
/// `[real]` half's job, and `equalsTranscript` is what ties the two together. Everything it calls
/// is the real thing; only the control flow is here.
fn transcribe_run<W: Write>(out: &mut W, board: &mut Board, settings: &RouterSettings) -> bool {
    use fr_board::StopConnectionOption;
    use fr_router::pipeline::batch_loop::{
        BOARD_RANK_LIMIT, FANOUT_RECOVERY_STAGNATION_PASSES, MAXIMUM_TRIES_ON_THE_SAME_BOARD,
        STAGNATION_PASS_LIMIT, STAGNATION_SCORE_THRESHOLD, STOP_AT_PASS_MINIMUM,
        STOP_AT_PASS_MODULO,
    };
    use fr_router::pipeline::{BatchAutorouter, BatchFanout, BoardHistory, RoutingFailureLog};
    use fr_router::score::BoardStatistics;

    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    let mut router = BatchAutorouter::for_routing_job(board, settings, RouterBudget::disabled());
    let scoring = settings.scoring.as_ref().expect("a scoring block");

    // :44-50.
    let mut any_routable = false;
    for i in 0..settings.get_layer_count() {
        if settings.get_layer_active(i) && board.layer_structure().layers[i].is_signal {
            any_routable = true;
            break;
        }
    }
    writeln!(out, "INIT anyRoutable={any_routable}").expect("write");
    if !any_routable {
        // :51-56.
        writeln!(out, "THROW IllegalArgumentException state=CANCELLED").expect("write");
        return false;
    }

    // :58-59.
    writeln!(out, "STATE STARTED").expect("write");

    // :62-63.
    router.initial_unrouted_count =
        i32::try_from(BatchAutorouter::calculate_incomplete_count(board)).unwrap_or(i32::MAX);
    writeln!(out, "INITIAL-UNROUTED {}", router.initial_unrouted_count).expect("write");

    // :65.
    let mut bh = BoardHistory::new(scoring);

    // :89-218 — the fanout pre-pass; off in mode `router-only`, and from Plan 7 Task 12 the real
    // `BatchFanout::fanout_board` in mode `router+fanout`.
    writeln!(
        out,
        "FANOUT enabled={} smdPins={}",
        settings.is_fanout_enabled(),
        board.get_smd_pins().len()
    )
    .expect("write");
    if settings.is_fanout_enabled() && !board.get_smd_pins().is_empty() {
        // :123-172.
        let fanout_summary =
            BatchFanout::fanout_board(board, settings, &stop, RouterBudget::disabled(), &mut sink)
                .expect("fanout_board answers Ok on every path");
        // :173.
        router.fanout_timed_out = fanout_summary.is_timed_out;
        let final_escape = fanout_summary.escape_statistics;
        writeln!(
            out,
            "FANOUT-SUMMARY completedPassCount={} isTimedOut={} escapeTotalSmdPins={} \
             escapedCount={} escapedPercentage={} fanoutTimedOut={} {}",
            fanout_summary.completed_pass_count,
            fanout_summary.is_timed_out,
            final_escape.total_smd_pins,
            final_escape.escaped_count,
            fr_dsn::java_double_to_string(final_escape.escaped_percentage),
            router.fanout_timed_out,
            p7t_common::board_shape(board),
        )
        .expect("write");
    }

    // :220.
    let current_unrouted = BatchAutorouter::calculate_incomplete_count(board);
    // :221-223.
    let is_router_enabled =
        settings.get_run_router() && settings.max_passes.is_none_or(|max| max >= 0);
    writeln!(
        out,
        "ROUTER-ENABLED {is_router_enabled} unrouted={current_unrouted}"
    )
    .expect("write");
    // :234.
    let mut continue_autorouting = is_router_enabled;

    // :236-242.
    let mut current_pass: i32 = 1;
    let mut consecutive_no_improvement_passes: i32 = 0;
    let mut fanout_recovery_applied = false;
    let mut last_best_score = f32::NEG_INFINITY;
    let mut global_best_score = f32::NEG_INFINITY;
    let mut pass_of_best_score: i32 = 0;
    let mut incomplete_count_at_best_score: usize = 0;
    // :249 — the dead `alreadyRoutedBoardHashes`; not transcribed and not ported (quirk #216).

    let mut failure_log = RoutingFailureLog::new();

    // :250.
    while continue_autorouting && !stop.is_stop_auto_router_requested() {
        // :251-253 — dead in production (quirk #203).
        if stop.poll_deadline() {
            stop.request_stop_auto_router();
        }

        // :268-273.
        if settings
            .max_passes
            .is_some_and(|max| max > 0 && current_pass > max)
        {
            writeln!(
                out,
                "MAXPASSES-BREAK pass={current_pass} maxPasses={}",
                settings.max_passes.expect("tested above")
            )
            .expect("write");
            stop.request_stop_auto_router();
            break;
        }

        // :275-280.
        writeln!(out, "STATE RUNNING pass={current_pass}").expect("write");

        // :282-283.
        let board_score_before = BoardStatistics::new(board).normalized_score(scoring);
        // :284.
        bh.add(board);
        writeln!(
            out,
            "HIST-ADD pass={current_pass} scoreBefore={} size={} maxScore={}",
            java_float_to_string(board_score_before),
            bh.size(),
            java_float_to_string(bh.max_score())
        )
        .expect("write");

        // :293.
        continue_autorouting = router
            .autoroute_pass(board, &mut failure_log, current_pass, &stop, &mut sink)
            .expect("run_single_thread's boundary answers Ok on every path");

        // :295-296.
        let mut board_statistics_after = BoardStatistics::new(board);
        let mut board_score_after = board_statistics_after.normalized_score(scoring);
        writeln!(
            out,
            "PASS-RET pass={current_pass} continueAutorouting={continue_autorouting} \
             scoreAfter={}",
            java_float_to_string(board_score_after)
        )
        .expect("write");

        // :298-344.
        let size_gate = i32::try_from(bh.size()).unwrap_or(i32::MAX) >= STOP_AT_PASS_MINIMUM
            || stop.is_stop_auto_router_requested();
        let mod_gate = ((current_pass % STOP_AT_PASS_MODULO == 0)
            && (current_pass >= STOP_AT_PASS_MINIMUM))
            || stop.is_stop_auto_router_requested();
        writeln!(
            out,
            "RESTORE-GATE pass={current_pass} sizeGate={size_gate} modGate={mod_gate} \
             maxScore={} fires={}",
            java_float_to_string(bh.max_score()),
            size_gate && mod_gate && bh.max_score() > board_score_after
        )
        .expect("write");
        // The two gates stay nested, as Java's `:298` and `:299` are, because this half of the
        // driver is a *transcription*: collapsing them would make the driver read differently from
        // the method it is evidence for. (The port's own `run` uses `restore_gate`, which is the
        // conjunction with its two Java ranges named.)
        #[allow(clippy::collapsible_if)]
        if size_gate {
            if mod_gate {
                // :306.
                if bh.max_score() > board_score_after {
                    // :307.
                    let Some(board_to_restore) = bh.restore_board(MAXIMUM_TRIES_ON_THE_SAME_BOARD)
                    else {
                        // :308-313.
                        writeln!(out, "RESTORE-NULL-BREAK pass={current_pass}").expect("write");
                        stop.request_stop_auto_router();
                        break;
                    };

                    // :315.
                    let board_to_restore_rank = bh.rank(&board_to_restore);
                    writeln!(
                        out,
                        "RESTORE-RANK pass={current_pass} rank={board_to_restore_rank}"
                    )
                    .expect("write");

                    // :317-320.
                    if board_to_restore_rank > i32::try_from(BOARD_RANK_LIMIT).unwrap_or(i32::MAX) {
                        writeln!(
                            out,
                            "RANK-BREAK pass={current_pass} rank={board_to_restore_rank}"
                        )
                        .expect("write");
                        stop.request_stop_auto_router();
                        break;
                    }

                    // :322-334.
                    *board = board_to_restore;
                    let board_statistics = BoardStatistics::new(board);
                    consecutive_no_improvement_passes = 0;
                    board_statistics_after = board_statistics;
                    board_score_after = board_statistics_after.normalized_score(scoring);
                    last_best_score = board_score_after;
                    writeln!(
                        out,
                        "RESTORED pass={current_pass} scoreAfter={}",
                        java_float_to_string(board_score_after)
                    )
                    .expect("write");
                }
            }
        }

        // :353-376 — the pass-completed report, i.e. the port's `PassRecord`.
        writeln!(
            out,
            "PASS pass={current_pass} score={} incompletes={} violations={} vias={} traces={}",
            java_float_to_string(board_score_after),
            stat(board_statistics_after.connections.incomplete_count),
            stat(board_statistics_after.clearance_violations.total_count),
            stat(board_statistics_after.items.via_count),
            stat(board_statistics_after.items.trace_count)
        )
        .expect("write");

        // :408-410.
        if settings.save_intermediate_stages == Some(true) {
            writeln!(out, "SNAPSHOT pass={current_pass}").expect("write");
        }

        // :422.
        if current_pass >= STOP_AT_PASS_MINIMUM && continue_autorouting {
            // :425-427.
            if board_score_after > last_best_score + STAGNATION_SCORE_THRESHOLD {
                consecutive_no_improvement_passes = 0;
                last_best_score = board_score_after;
            } else {
                // :429.
                consecutive_no_improvement_passes += 1;

                // :435-454 — never fires in `router-only`.
                if settings.is_fanout_enabled()
                    && !fanout_recovery_applied
                    && stat(board_statistics_after.connections.incomplete_count) > 0
                    && consecutive_no_improvement_passes >= FANOUT_RECOVERY_STAGNATION_PASSES
                {
                    writeln!(out, "FANOUT-RECOVERY pass={current_pass}").expect("write");
                    router
                        .remove_tails(board, None, StopConnectionOption::None, &|| {
                            stop.is_stop_requested()
                        })
                        .expect("removeTails");
                    board_statistics_after = BoardStatistics::new(board);
                    board_score_after = board_statistics_after.normalized_score(scoring);
                    last_best_score = board_score_after;
                    consecutive_no_improvement_passes = 0;
                    fanout_recovery_applied = true;
                }

                // :456-476.
                if consecutive_no_improvement_passes >= STAGNATION_PASS_LIMIT {
                    writeln!(
                        out,
                        "STAGNATION-LOCAL-BREAK pass={current_pass} \
                         counter={consecutive_no_improvement_passes}"
                    )
                    .expect("write");
                    stop.request_stop_auto_router();
                    break;
                }
            }

            // :482-485.
            if board_score_after > global_best_score + STAGNATION_SCORE_THRESHOLD {
                global_best_score = board_score_after;
                pass_of_best_score = current_pass;
                incomplete_count_at_best_score =
                    stat(board_statistics_after.connections.incomplete_count);
            } else if (current_pass - pass_of_best_score) >= STAGNATION_PASS_LIMIT {
                // :486-507.
                writeln!(
                    out,
                    "STAGNATION-GLOBAL-BREAK pass={current_pass} \
                     passOfBestScore={pass_of_best_score} \
                     incompleteAtBest={incomplete_count_at_best_score}"
                )
                .expect("write");
                stop.request_stop_auto_router();
                break;
            }
        } else if stat(board_statistics_after.connections.incomplete_count) == 0
            && board_score_after > STAGNATION_SCORE_THRESHOLD
        {
            // :509-517 — quirk #215.
            writeln!(out, "ROUTED-RESET pass={current_pass}").expect("write");
            consecutive_no_improvement_passes = 0;
            last_best_score = board_score_after;
        }

        writeln!(
            out,
            "STAGNATION pass={current_pass} counter={consecutive_no_improvement_passes} \
             lastBest={} globalBest={} passOfBest={pass_of_best_score}",
            java_float_to_string(last_best_score),
            java_float_to_string(global_best_score)
        )
        .expect("write");

        // :520-522.
        if continue_autorouting && !stop.is_stop_auto_router_requested() {
            current_pass += 1;
        }
    }

    // :528-530.
    let current_final_score = BoardStatistics::new(board).normalized_score(scoring);
    let best_history_score = bh.max_score();
    let mut swapped = false;
    // :531-550.
    if best_history_score > current_final_score {
        if let Some(best_board) = bh.restore_best_board() {
            *board = best_board;
            swapped = true;
        }
    }
    writeln!(
        out,
        "FINAL-SWAP currentFinalScore={} bestHistoryScore={} swapped={swapped}",
        java_float_to_string(current_final_score),
        java_float_to_string(best_history_score)
    )
    .expect("write");

    // :552 — `job.board = router.board`; the port's `board` is that field.

    // :554-563.
    let was_router_run =
        settings.get_run_router() && settings.max_passes.is_none_or(|max| max >= 0);
    let tails_fire = was_router_run
        && !(router.is_remove_unconnected_vias()
            || continue_autorouting
            || stop.is_stop_auto_router_requested());
    writeln!(
        out,
        "TAILS wasRouterRun={was_router_run} removeUnconnectedVias={} \
         continueAutorouting={continue_autorouting} stopAutoRouterRequested={} fires={tails_fire}",
        router.is_remove_unconnected_vias(),
        stop.is_stop_auto_router_requested()
    )
    .expect("write");
    if tails_fire {
        router
            .remove_tails(board, None, StopConnectionOption::None, &|| {
                stop.is_stop_requested()
            })
            .expect("removeTails");
    }

    // :565.
    bh.clear();

    // :571-585.
    let state = if !stop.is_stop_auto_router_requested() {
        TaskState::Finished
    } else if stop.is_timed_out() {
        TaskState::TimedOut
    } else {
        TaskState::Cancelled
    };
    writeln!(
        out,
        "RESULT state={} passesRun={current_pass} continueRouting={} stopRequested={} \
         stopAutoRouterRequested={}",
        java_task_state(state),
        !stop.is_stop_auto_router_requested(),
        stop.is_stop_requested(),
        stop.is_stop_auto_router_requested()
    )
    .expect("write");

    // :587.
    !stop.is_stop_auto_router_requested()
}

/// Java's `TaskState.toString()`, i.e. the enum constant's own name.
fn java_task_state(state: TaskState) -> &'static str {
    match state {
        TaskState::Idle => "IDLE",
        TaskState::Started => "STARTED",
        TaskState::Running => "RUNNING",
        TaskState::Finished => "FINISHED",
        TaskState::Cancelled => "CANCELLED",
        TaskState::TimedOut => "TIMED_OUT",
    }
}

/// `BoardStatistics`' `Option<i32>` counters, as `AutorouteBatchLoop`'s own `stat` reads them.
fn stat(value: Option<i32>) -> usize {
    usize::try_from(value.expect("the computing constructor fills every count")).unwrap_or(0)
}

// ------------------------------------------------------------------------------------------------
// The optimizer stage — BatchOptimizer.java:125-272, :279-385 (Plan 7 Task 14)
// ------------------------------------------------------------------------------------------------

/// `P7T9.runOptimizerMode` — the whole `-dr`-equivalent run, router stage **and** optimizer stage,
/// in this driver's two halves. See `P7T9.java`'s method comment for the stop-flag seam that makes
/// `optimizer-shared` a different program from `optimizer`, and for why the transcription computes
/// `:335-338`'s statistics unconditionally.
fn run_optimizer_mode<W: Write>(
    out: &mut W,
    dsn: &std::path::Path,
    max_passes: i32,
    mode: &str,
    opt_passes: Option<i32>,
    opt_items: Option<i32>,
) {
    let shared = mode == "optimizer-shared";

    // ---- half one: the transcription ---------------------------------------------------------
    let mut board = p7t_common::load_board(dsn);
    let mut settings = build_settings(&board, max_passes, mode);
    apply_optimizer_limits(&mut settings, opt_passes, opt_items);
    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    writeln!(out, "[route]").expect("write");
    let routed: BatchLoopResult = AutorouteBatchLoop::run(
        &mut board,
        &settings,
        &stop,
        RouterBudget::disabled(),
        &mut sink,
    )
    .expect("the corpus stems all have a routable signal layer");
    writeln!(
        out,
        "ROUTED returned={} {}",
        routed.continue_routing,
        p7t_common::board_shape(&mut board)
    )
    .expect("write");
    writeln!(
        out,
        "SEAM shared={shared} stopRequested={} stopAutoRouterRequested={}",
        stop.is_stop_requested(),
        stop.is_stop_auto_router_requested()
    )
    .expect("write");

    // `BatchOptimizer.createForHeadless` (`:51-53`). A fresh `RouterStop` is `P7T9.newOptimizer`'s
    // fresh `NeverStarted`; the shared one is the prologue's own flag, i.e. what
    // `RoutingPipeline.run` hands the stage.
    let fresh = RouterStop::new();
    let optimizer_stop: &RouterStop = if shared { &stop } else { &fresh };
    let mut optimizer = BatchOptimizer::new(&settings);
    writeln!(out, "[transcript]").expect("write");
    transcribe_optimizer(out, &mut optimizer, &mut board, &settings, optimizer_stop);
    let transcript_hash = board.structural_hash();

    // ---- half two: the real method -------------------------------------------------------------
    let mut real_board = p7t_common::load_board(dsn);
    let mut real_settings = build_settings(&real_board, max_passes, mode);
    apply_optimizer_limits(&mut real_settings, opt_passes, opt_items);
    let real_stop = RouterStop::new();
    let mut real_sink = NoopProgressSink;
    writeln!(out, "[real]").expect("write");
    let real_routed: BatchLoopResult = AutorouteBatchLoop::run(
        &mut real_board,
        &real_settings,
        &real_stop,
        RouterBudget::disabled(),
        &mut real_sink,
    )
    .expect("the corpus stems all have a routable signal layer");
    writeln!(
        out,
        "REAL-ROUTED returned={} {}",
        real_routed.continue_routing,
        p7t_common::board_shape(&mut real_board)
    )
    .expect("write");
    let real_fresh = RouterStop::new();
    let real_optimizer_stop: &RouterStop = if shared { &real_stop } else { &real_fresh };
    let mut real_optimizer = BatchOptimizer::new(&real_settings);
    let real_result = real_optimizer
        .run_batch_loop(
            &mut real_board,
            real_optimizer_stop,
            RouterBudget::disabled(),
            &mut real_sink,
        )
        .expect("the optimizer stage cannot fail on the corpus");
    writeln!(
        out,
        "OPT-REAL items={} timedOut={} useIncreasedRipupCosts={} {} equalsTranscript={}",
        real_result.items_optimized,
        real_result.timed_out,
        real_optimizer.use_increased_ripup_costs,
        p7t_common::board_shape(&mut real_board),
        real_board.structural_hash() == transcript_hash
    )
    .expect("write");

    // ---- the final board, in `P6T15aProbe`'s polyline format ------------------------------------
    writeln!(out, "[board]").expect("write");
    p7t_common::dump_board(out, &board);
}

/// `P7T9.applyOptimizerLimits`.
fn apply_optimizer_limits(
    settings: &mut RouterSettings,
    opt_passes: Option<i32>,
    opt_items: Option<i32>,
) {
    let optimizer = settings.optimizer.get_or_insert_with(Default::default);
    optimizer.max_passes = opt_passes;
    optimizer.max_items = opt_items;
    // Ruling AI: `:153-160` builds the stage's deadline from this string, and no corpus run sets
    // it. Cleared explicitly so a settings source cannot make the run wall-clock dependent.
    optimizer.timeout_string = None;
}

/// `P7T9.transcribeOptimizer`: `runBatchLoop`'s body (`BatchOptimizer.java:125-272`) written out
/// against the port's own primitives, calling the real `BoardStatistics`, the real
/// `normalized_score` and — through [`transcribe_opt_route_pass`] — the real
/// [`BatchOptimizer::opt_route_item`].
fn transcribe_optimizer<W: Write>(
    out: &mut W,
    optimizer: &mut BatchOptimizer<'_>,
    board: &mut Board,
    settings: &RouterSettings,
    stop: &RouterStop,
) {
    let scoring = settings.scoring.as_ref().expect("the scoring block");
    let optimizer_settings = settings.optimizer.as_ref().expect("the optimizer block");
    let threshold = optimizer_settings
        .optimization_improvement_threshold
        .expect("the threshold");
    // :132.
    optimizer.use_increased_ripup_costs = true;
    // :135-138.
    let initial_stats = BoardStatistics::new(board);
    writeln!(
        out,
        "OPT-START score={} incompletes={} violations={} maxPasses={} maxItems={} threshold={} \
         maxConsecutiveFailures={}",
        java_float_to_string(initial_stats.normalized_score(scoring)),
        stat(initial_stats.connections.incomplete_count),
        stat(initial_stats.clearance_violations.total_count),
        limit_name(optimizer_settings.max_passes),
        limit_name(optimizer_settings.max_items),
        java_float_to_string(threshold),
        optimizer_settings
            .max_consecutive_failures
            .map_or_else(|| "null".to_string(), |value| value.to_string()),
    )
    .expect("write");
    // :153-160 — `timeoutString` is `None` on every corpus run.
    writeln!(
        out,
        "OPT-DEADLINE timeoutString={}",
        optimizer_settings
            .timeout_string
            .as_deref()
            .unwrap_or("null")
    )
    .expect("write");
    // :162-163.
    writeln!(out, "OPT-STATE STARTED").expect("write");

    let mut current_pass = 0_i32;
    // :167-171 — `ALL` only.
    while optimizer_settings
        .max_passes
        .is_none_or(|max_passes| current_pass < max_passes)
        && optimizer_settings
            .max_items
            .is_none_or(|max_items| optimizer.total_items_optimized < max_items)
        && !stop.is_stop_requested()
    {
        // :172-176 — the per-stage deadline; unreachable here, see `OPT-DEADLINE`.
        // :177.
        current_pass += 1;
        // :179.
        let score_before_pass = BoardStatistics::new(board).normalized_score(scoring);
        // :182-193.
        if score_before_pass * (1.0 + threshold) >= 1000.0 {
            writeln!(
                out,
                "OPT-STOP reason=near-perfect pass={current_pass} score={}",
                java_float_to_string(score_before_pass)
            )
            .expect("write");
            break;
        }
        // :195-198.
        writeln!(out, "OPT-STATE RUNNING pass={current_pass}").expect("write");
        // :200.
        let with_preferred_directions = current_pass % 2 != 0;
        // :201.
        let route_improved = transcribe_opt_route_pass(
            out,
            optimizer,
            board,
            settings,
            current_pass,
            with_preferred_directions,
            stop,
        );
        // :204-206.
        if optimizer.is_timed_out() {
            writeln!(out, "OPT-STOP reason=timeout pass={current_pass}").expect("write");
            break;
        }
        // :208.
        let statistics_after = BoardStatistics::new(board);
        let score_after_pass = statistics_after.normalized_score(scoring);
        // :209-218 — the port's own arm, so the transcription cannot drift from it.
        let (pass_improvement, force_another_pass) =
            optimizer.apply_pass_improvement(score_before_pass, score_after_pass);
        writeln!(
            out,
            "OPT-PASS pass={current_pass} withPreferredDirections={with_preferred_directions} \
             scoreBefore={} scoreAfter={} passImprovement={} scoreImprovement={} \
             useIncreasedRipupCosts={} routeImproved={} items={} pass={current_pass} score={} \
             incompletes={} violations={} vias={} traces={}",
            java_float_to_string(score_before_pass),
            java_float_to_string(score_after_pass),
            java_double_to_string(pass_improvement),
            // fixed: T9 (#228) — Java's `-1` sentinel is a `bool` here; the transcript keeps
            // printing the number Java printed so the two sides stay comparable.
            java_double_to_string(if force_another_pass {
                -1.0
            } else {
                pass_improvement
            }),
            optimizer.use_increased_ripup_costs,
            java_float_to_string(route_improved),
            optimizer.total_items_optimized,
            java_float_to_string(score_after_pass),
            stat(statistics_after.connections.incomplete_count),
            stat(statistics_after.clearance_violations.total_count),
            stat(statistics_after.items.via_count),
            stat(statistics_after.items.trace_count),
        )
        .expect("write");
        // :220-230.
        if !force_another_pass && pass_improvement < f64::from(threshold) {
            writeln!(
                out,
                "OPT-STOP reason=threshold pass={current_pass} scoreImprovement={}",
                java_double_to_string(pass_improvement)
            )
            .expect("write");
            break;
        }
    }

    // :233-234 — unconditional, whatever ended the loop.
    writeln!(out, "OPT-STATE FINISHED pass={current_pass}").expect("write");
    // :250-251.
    let final_stats = BoardStatistics::new(board);
    // :252-255.
    let state = if optimizer.is_timed_out() {
        "TIMED_OUT"
    } else if stop.is_stop_requested() {
        "CANCELLED"
    } else {
        "FINISHED"
    };
    writeln!(
        out,
        "OPT-RESULT state={state} passesRun={current_pass} items={} timedOut={} \
         useIncreasedRipupCosts={} finalScore={} {}",
        optimizer.total_items_optimized,
        optimizer.is_timed_out(),
        optimizer.use_increased_ripup_costs,
        java_float_to_string(final_stats.normalized_score(scoring)),
        p7t_common::board_shape(board)
    )
    .expect("write");
}

/// `P7T9.transcribeOptRoutePass`: `optRoutePass`' body (`BatchOptimizer.java:279-385`), calling the
/// real [`BatchOptimizer::opt_route_item`] once per item.
#[allow(clippy::too_many_arguments)]
fn transcribe_opt_route_pass<W: Write>(
    out: &mut W,
    optimizer: &mut BatchOptimizer<'_>,
    board: &mut Board,
    settings: &RouterSettings,
    pass_no: i32,
    with_preferred_directions: bool,
    stop: &RouterStop,
) -> f32 {
    let optimizer_settings = settings.optimizer.as_ref().expect("the optimizer block");
    // :281.
    let board_statistics_before = BoardStatistics::new(board);
    // :284.
    optimizer.progress_throttler.reset();
    // :287.
    optimizer.sorted_route_items = Some(ReadSortedRouteItems::new());
    // :288.
    optimizer.min_cumulative_trace_length = f64::from(
        board_statistics_before
            .traces
            .total_weighted_length
            .unwrap_or(0.0),
    );
    // :300-304.
    let mut consecutive_failures = 0_i32;
    let max_consecutive_failures = optimizer_settings.max_consecutive_failures.unwrap_or(50);
    // :306.
    let mut route_improved = 0.0_f32;
    let mut n = 0_i32;
    let mut sink = NoopProgressSink;
    // :307.
    loop {
        // :308-313 — no deadline on this run.
        // :314-317.
        if stop.is_stop_requested() {
            writeln!(out, "OPT-PASS-STOP pass={pass_no} reason=stop n={n}").expect("write");
            return route_improved;
        }
        // :318-326.
        if optimizer_settings
            .max_items
            .is_some_and(|max_items| max_items > 0 && optimizer.total_items_optimized >= max_items)
        {
            writeln!(out, "OPT-PASS-STOP pass={pass_no} reason=max-items n={n}").expect("write");
            break;
        }
        // :327-330.
        let Some(current_item) = reader_next(optimizer, board) else {
            writeln!(out, "OPT-PASS-STOP pass={pass_no} reason=exhausted n={n}").expect("write");
            break;
        };
        let kind = p7t_common::java_class_name(
            board
                .get_item(current_item)
                .expect("the reader returns live items"),
        );
        // :331.
        let result: ItemRouteResult = optimizer
            .opt_route_item(
                board,
                current_item,
                with_preferred_directions,
                false,
                stop,
                RouterBudget::disabled(),
                &mut sink,
            )
            .expect("optRouteItem cannot fail on the corpus");
        // :332.
        optimizer.total_items_optimized += 1;
        let mut broke = false;
        // :333-348.
        if result.improved() {
            consecutive_failures = 0;
            // :335-338, ungated — see `P7T9.java`'s progress-throttle section.
            BoardStatistics::new(board);
            route_improved = optimizer_route_improved(
                &result,
                board_statistics_before.items.via_count.unwrap_or(0),
                board_statistics_before.traces.total_length.unwrap_or(0.0),
            );
        } else {
            // :350-360.
            consecutive_failures += 1;
            broke = consecutive_failures >= max_consecutive_failures;
        }
        writeln!(
            out,
            "OPT-ITEM pass={pass_no} n={n} id={} kind={kind} improved={} viaCount={} \
             traceLength={} incompleteBefore={} incompleteAfter={} improvementPercentage={} \
             routeImproved={} consecutiveFailures={consecutive_failures} items={} \
             minCumulativeTraceLength={}",
            current_item.0,
            result.improved(),
            result.via_count(),
            java_double_to_string(result.trace_length()),
            result.incomplete_count_before(),
            result.incomplete_count(),
            java_float_to_string(result.improvement_percentage()),
            java_float_to_string(route_improved),
            optimizer.total_items_optimized,
            java_double_to_string(optimizer.min_cumulative_trace_length),
        )
        .expect("write");
        n += 1;
        if broke {
            writeln!(
                out,
                "OPT-PASS-STOP pass={pass_no} reason=consecutive-failures n={n}"
            )
            .expect("write");
            break;
        }
    }
    // :364.
    optimizer.sorted_route_items = None;
    // :365-368.
    if optimizer.use_increased_ripup_costs && route_improved == 0.0 {
        optimizer.use_increased_ripup_costs = false;
        route_improved = -1.0;
    }
    // :371-372.
    BoardStatistics::new(board);
    // :384.
    route_improved
}

/// `optimizer.sortedRouteItems`' aliasing, spelled out — `p7t8.rs`'s helper, for the same reason:
/// the field is a `Copy` value here and a live inner-class instance in Java.
fn reader_next(optimizer: &mut BatchOptimizer<'_>, board: &Board) -> Option<ItemId> {
    let mut reader = optimizer
        .sorted_route_items
        .expect("optRoutePass:287 assigned it");
    let result = reader.next(board);
    optimizer.sorted_route_items = Some(reader);
    result
}
