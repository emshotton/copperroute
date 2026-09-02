use crate::cli::RouteArgs;
use crate::legacy::ExitCode;

/// # Obligation
///
// obligation: Plan 8 Task 6 wires this to `fr_core`'s load/pipeline/save sequence and to
//   `fr_settings::resolve_headless` over `settings_argv`; until then the run answers
//   [`ExitCode::NotImplemented`], which Task 12 must make unreachable.
pub fn run(args: &RouteArgs, settings_argv: &[String]) -> ExitCode {
    let _ = settings_argv;
    tracing::error!(
        "route: not implemented yet (Plan 8 Task 6): {}",
        args.input.display()
    );
    ExitCode::NotImplemented
}
