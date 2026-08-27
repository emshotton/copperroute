mod cli;
mod commands;
mod legacy;
mod mcp;

use clap::Parser;

fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let argv = match legacy::rewrite(&raw) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(2);
        }
    };
    let cli = cli::Cli::parse_from(std::iter::once("freerouting".to_string()).chain(argv));
    init_logging(cli.verbose, cli.log_level.as_deref());
    let code = match &cli.command {
        cli::Command::Route(a) => commands::route::run(a),
        cli::Command::Drc(a) => commands::drc::run(a),
        cli::Command::Info(a) => commands::info::run(a),
        cli::Command::Mcp => mcp::stdio::run(),
    };
    std::process::exit(code);
}

fn init_logging(verbose: u8, level: Option<&str>) {
    let filter = match (level, verbose) {
        (Some(l), _) => l.to_string(),
        (None, 0) => "warn".into(),
        (None, 1) => "info".into(),
        (None, 2) => "debug".into(),
        (None, _) => "trace".into(),
    };
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(filter))
        .with_writer(std::io::stderr)
        .init();
}
