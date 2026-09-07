use std::collections::BTreeSet;
use std::time::Instant;

use copper_board::{FixedState, Item};
use copper_router::pipeline::{BatchOptimizer, NoopProgressSink, RouterBudget, RouterStop};
use copper_router::score::BoardStatistics;
use copper_settings::sources::DsnFileSettings;
use copper_settings::{HostEnvironment, SettingsInputs, SettingsSource};

fn quality(stats: &BoardStatistics) -> serde_json::Value {
    serde_json::json!({
        "unrouted": stats.connections.incomplete_count,
        "vias": stats.items.via_count,
        "length": stats.traces.total_length,
        "violations": stats.clearance_violations.total_count,
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if !(5..=6).contains(&args.len()) {
        return Err(
            "usage: optimizer_bench INPUT.dsn INPUT.ses MAX_ITEMS DEADLINE_MS [REPEAT_WINDOW_MS]"
                .into(),
        );
    }
    let repeat_window_ms: u128 = args.get(5).map(|s| s.parse()).transpose()?.unwrap_or(0);
    let started = Instant::now();
    for _ in 0..1000 {
        run_once(&args)?;
        if started.elapsed().as_millis() >= repeat_window_ms {
            break;
        }
    }
    Ok(())
}

fn run_once(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = std::fs::read(&args[1])?;
    let copper_core::ParsedBoard {
        mut board,
        transform,
        warnings,
        ..
    } = copper_core::parse_board_result(copper_dsn::read_board(
        &bytes[..],
        None,
        Some(&args[1]),
        &copper_dsn::DsnReadOptions::default(),
    ))?;
    for warning in warnings {
        eprintln!("{warning}");
    }
    let dsn = DsnFileSettings::new(&bytes[..], &args[1]);
    let mut settings = copper_settings::resolve_headless(
        &SettingsInputs {
            dsn: dsn.get_settings(),
            ..Default::default()
        },
        Some(&board),
        &HostEnvironment::detect(),
    );
    let optimizer_settings = settings
        .optimizer
        .as_mut()
        .ok_or("missing optimizer settings")?;
    optimizer_settings.max_threads = Some(1);
    optimizer_settings.max_items = Some(args[3].parse()?);
    optimizer_settings.max_passes = Some(1);
    if let Ok(limit) = std::env::var("COPPERROUTE_BENCH_SEARCH_STEPS") {
        optimizer_settings.max_search_steps = Some(limit.parse()?);
    }
    copper_core::prepare_board(&mut board, &settings);
    copper_core::apply_immediate_post_load_processing(&mut board);
    let original: BTreeSet<_> = board.items_in_board_order().into_iter().collect();
    let imported =
        copper_dsn::ses_reader::read(std::fs::File::open(&args[2])?, &mut board, &transform)?;
    if imported.errors_encountered != 0 {
        return Err(format!("session import had {} errors", imported.errors_encountered).into());
    }
    let mut unlocked = 0;
    for id in board.items_in_board_order() {
        if !original.contains(&id)
            && let Some(item @ (Item::Trace(_) | Item::Via(_))) = board.get_item_mut(id)
        {
            item.header_mut().set_fixed_state(FixedState::Unfixed);
            unlocked += 1;
        }
    }
    let before = BoardStatistics::new(&mut board);
    let mut optimizer = BatchOptimizer::new(&settings);
    let stop = RouterStop::with_deadline(args[4].parse()?);
    eprintln!("OPTIMIZER_START unlocked={unlocked}");
    let started = Instant::now();
    let result = optimizer.run_batch_loop(
        &mut board,
        &stop,
        RouterBudget::default(),
        &mut NoopProgressSink,
    );
    let elapsed = started.elapsed().as_secs_f64();
    let after = BoardStatistics::new(&mut board);
    if let Ok(path) = std::env::var("COPPERROUTE_BENCH_OUTPUT") {
        copper_dsn::ses_writer::write(
            &board,
            &transform,
            &mut std::fs::File::create(path)?,
            "benchmark",
        )?;
    }
    println!(
        "{}",
        serde_json::json!({
            "optimizer_s": elapsed,
            "unlocked_items": unlocked,
            "items_attempted": optimizer.total_items_optimized,
            "route_work": optimizer.total_route_work,
            "search_steps": optimizer.search_work_budget.as_ref().map(|work| work.spent()),
            "timed_out": stop.is_timed_out(),
            "error": result.as_ref().err().map(ToString::to_string),
            "passes": result.as_ref().ok().map(|result| result.passes_run),
            "before": quality(&before),
            "after": quality(&after),
        })
    );
    Ok(())
}
