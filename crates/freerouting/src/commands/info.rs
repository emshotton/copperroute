use crate::cli::InfoArgs;
use crate::legacy::ExitCode;

/// # Obligation
///
// obligation: Plan 8 Task 12 wires this to `fr_core`'s load sequence and to
//   `fr_settings::resolve_headless` over `settings_argv`; until then the run answers
//   [`ExitCode::NotImplemented`], which Task 12 must make unreachable. *(This marker named
//   Task 6 until Task 6 landed: Task 6 owns `route` only, and it left the shape — the
//   `parse_board_if_needed` / one-`resolve_headless` / post-load-passes order — for this runner
//   to follow. See `commands/route.rs`'s module docs.)*
pub fn run(args: &InfoArgs, settings_argv: &[String]) -> ExitCode {
    let _ = settings_argv;
    tracing::error!(
        "info: not implemented yet (Plan 8 Task 12): {}",
        args.input.display()
    );
    ExitCode::NotImplemented
}
