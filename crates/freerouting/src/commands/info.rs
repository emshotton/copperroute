use crate::cli::InfoArgs;

pub fn run(args: &InfoArgs) -> i32 {
    tracing::error!(input = %args.input.display(), "info: not implemented yet (Plan 8)");
    super::EXIT_NOT_IMPLEMENTED
}
