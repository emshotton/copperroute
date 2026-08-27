use crate::cli::RouteArgs;

pub fn run(args: &RouteArgs) -> i32 {
    tracing::error!(input = %args.input.display(), "route: not implemented yet (Plan 8)");
    super::EXIT_NOT_IMPLEMENTED
}
