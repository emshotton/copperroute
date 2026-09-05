use crate::cli::InfoArgs;
use crate::legacy::ExitCode;
use fr_core::{RoutingJob, SessionId};

pub fn run(args: &InfoArgs, settings_argv: &[String]) -> ExitCode {
    let _ = settings_argv;

    let mut job = RoutingJob::new(SessionId::NIL);
    if let Err(error) = job.set_input(&args.input) {
        tracing::error!(
            "Couldn't load the input file '{}': {error}",
            args.input.display()
        );
        return ExitCode::Failure;
    }

    let loaded = match fr_core::load_board_if_needed(&mut job) {
        Ok(loaded) => loaded,
        Err(error) => {
            tracing::error!("{error}");
            return ExitCode::Failure;
        }
    };
    let mut board = loaded.board;
    for warning in &loaded.warnings {
        tracing::warn!("{warning}");
    }

    let summary = fr_core::summarise(&mut board, loaded.metadata.as_ref());
    println!("{}", summary.to_json_pretty());
    ExitCode::Ok
}
