use crate::ExitCode;
use crate::cli::{Cli, InfoArgs};
use crate::ops::info::{InfoRequest, info};
use crate::ops::load::{BoardSource, LoadRequest};

pub fn run(cli: &Cli, args: &InfoArgs) -> ExitCode {
    let mut load = LoadRequest::for_board(BoardSource::Path(args.input.clone()));
    load.settings = super::overrides(cli, &[]);
    match info(&InfoRequest { load }) {
        Ok(summary) => {
            println!("{}", summary.to_json_pretty());
            ExitCode::Ok
        }
        Err(error) => {
            tracing::error!("{error}");
            ExitCode::Failure
        }
    }
}
