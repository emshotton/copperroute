use crate::cli::DrcArgs;

pub fn run(args: &DrcArgs) -> i32 {
    tracing::error!(input = %args.input.display(), "drc: not implemented yet (Plan 8)");
    super::EXIT_NOT_IMPLEMENTED
}
