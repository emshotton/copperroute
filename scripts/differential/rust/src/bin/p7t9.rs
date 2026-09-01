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
use fr_dsn::java_float_to_string;
use fr_router::pipeline::{
    AutorouteBatchLoop, BatchLoopResult, NoopProgressSink, RouterBudget, RouterStop, TaskState,
};
use fr_settings::RouterSettings;

#[path = "../p7t_common.rs"]
mod p7t_common;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: p7t9 <dsn> [maxPasses] [mode]");
        std::process::exit(2);
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
    if mode != "router-only" && mode != "router+fanout" {
        eprintln!("p7t9: mode must be 'router-only' or 'router+fanout', not: {mode}");
        std::process::exit(2);
    }

    let stdout = std::io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    let (jar, bytes, mtime) = p7t_common::jar_identity();
    writeln!(
        out,
        "HEADER jar={jar} bytes={bytes} mtime={mtime} fixture={} maxPasses={max_passes} \
         mode={mode}",
        dsn.file_name().expect("a file name").to_string_lossy(),
    )
    .expect("write");
    if let Ok(exe) = std::env::current_exe() {
        eprintln!("rust-binary {}", exe.display());
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

/// `P7T9.buildSettings` — `p7t_common::build_settings` plus the driver's three knobs.
fn build_settings(board: &Board, max_passes: i32, mode: &str) -> RouterSettings {
    let mut settings = p7t_common::build_settings(board);
    settings.max_passes = Some(max_passes);
    // `RouterSettings.setFanoutEnabled(…)` (RouterSettings.java:583-591): the object must exist,
    // and `isFanoutEnabled` (`:578-580`) then answers the flag.
    let fanout = settings.fanout.get_or_insert_with(Default::default);
    fanout.enabled = Some(mode == "router+fanout");
    // Ruling AI, and the same knob `P7T9.buildSettings` writes: `fanoutPass:231-232` builds its
    // per-pin `TimeLimit` from this setting, so this is where the fanout stage's clock goes off.
    fanout.max_milliseconds_per_pin = Some(i64::from(i32::MAX));
    settings.set_run_router(true);
    settings.set_run_optimizer(false);
    settings.save_intermediate_stages = Some(false);
    settings
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
