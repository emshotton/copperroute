#![forbid(unsafe_code)]

pub mod cli;
pub mod commands;
pub mod legacy;
pub mod logging;
pub mod mcp;

use clap::Parser;

use legacy::{ExitCode, Level};

#[must_use]
pub fn run(raw: &[String]) -> ExitCode {
    logging::init(logging::level_from_argv(raw));

    let (argv, diagnostics) = legacy::rewrite(raw);
    for diagnostic in &diagnostics {
        match diagnostic.level {
            Level::Warn => tracing::warn!("{}", diagnostic.message),
            Level::Error => tracing::error!("{}", diagnostic.message),
        }
    }

    // Port only, at DEBUG: the native command line the legacy form was rewritten into. Java has
    tracing::debug!("rewritten command line: {argv:?}");

    if argv.is_empty() {
        return ExitCode::Failure;
    }

    let cli = match cli::Cli::try_parse_from(
        std::iter::once("freerouting".to_string()).chain(argv.iter().cloned()),
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

    match &cli.command {
        cli::Command::Route(args) => commands::route::run(args, raw),
        cli::Command::Drc(args) => commands::drc::run(args, raw),
        cli::Command::Info(args) => commands::info::run(args, raw),
        cli::Command::Mcp => match mcp::stdio::run(raw) {
            0 => ExitCode::Ok,
            _ => ExitCode::Failure,
        },
    }
}
