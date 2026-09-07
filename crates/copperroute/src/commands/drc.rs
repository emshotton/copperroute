use std::path::Path;

use crate::ExitCode;
use crate::cli::{Cli, DrcArgs};
use crate::ops::drc::{DrcRequest, drc, report_date};
use crate::ops::load::{BoardSource, LoadRequest};

pub fn run(cli: &Cli, args: &DrcArgs) -> ExitCode {
    let mut load = LoadRequest::for_board(BoardSource::Path(args.input.clone()));
    load.rules = args.rules.clone();
    load.session = args.ses.clone();
    load.kicad_project = args.kicad_project.clone();
    load.settings = super::overrides(cli, &[]);

    let outcome = match drc(&DrcRequest {
        load,
        date: report_date(std::time::SystemTime::now()),
    }) {
        Ok(outcome) => outcome,
        Err(error) => {
            tracing::error!("{error}");
            return ExitCode::Failure;
        }
    };

    match args.output.as_deref() {
        None => println!("{}", outcome.json),
        Some(path) => {
            if let Err(error) = write_report(path, &outcome.json) {
                tracing::error!(
                    "Couldn't save the DRC report to '{}': {error}",
                    path.display()
                );
                return ExitCode::Failure;
            }
            tracing::info!("DRC report written to {}", path.display());
        }
    }
    if outcome.violation_count > 0 {
        tracing::warn!("{} DRC violation(s) found", outcome.violation_count);
        ExitCode::Failure
    } else {
        ExitCode::Ok
    }
}

fn write_report(path: &Path, json: &str) -> std::io::Result<()> {
    std::fs::write(path, json.as_bytes())
}
