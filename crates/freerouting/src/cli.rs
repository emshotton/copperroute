use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "freerouting",
    version,
    about = "Headless PCB autorouter (Rust port of freerouting)"
)]
pub struct Cli {
    /// Increase log verbosity (-v, -vv)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,
    /// Explicit log level (error|warn|info|debug|trace); overrides -v
    #[arg(long, global = true)]
    pub log_level: Option<String>,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Route a Specctra DSN board and write a SES session file
    Route(RouteArgs),
    /// Run the design rule checker and emit a KiCad-schema JSON report
    Drc(DrcArgs),
    /// Print board summary (layers, nets, components) as JSON
    Info(InfoArgs),
    /// Run as an MCP server over stdio
    Mcp,
}

#[derive(Args, Debug)]
pub struct RouteArgs {
    /// Input .dsn file
    pub input: PathBuf,
    /// Output .ses (or .dsn) file
    #[arg(short, long)]
    pub output: PathBuf,
    #[arg(long)]
    pub rules: Option<PathBuf>,
    /// Previous session to load before routing (incremental)
    #[arg(long)]
    pub ses: Option<PathBuf>,
    /// KiCad JSON board file (legacy `-de …*.json`); not read yet — Plan 5/8 owns the loader
    #[arg(long)]
    pub kicad_json: Option<PathBuf>,
    #[arg(long)]
    pub max_passes: Option<u32>,
    /// Routing timeout in seconds
    #[arg(long)]
    pub timeout: Option<u64>,
    #[arg(long)]
    pub threads: Option<u32>,
    #[arg(long)]
    pub result_json: Option<PathBuf>,
    #[arg(long)]
    pub optimizer_improvement_threshold: Option<f64>,
    #[arg(long)]
    pub ignore_net_classes: Option<String>,
    #[arg(long)]
    pub update_strategy: Option<String>,
    #[arg(long)]
    pub hybrid_ratio: Option<String>,
    #[arg(long)]
    pub item_selection: Option<String>,
    /// Generic settings override, `section.field=value` (repeatable)
    #[arg(long = "set")]
    pub set: Vec<String>,
}

#[derive(Args, Debug)]
pub struct DrcArgs {
    pub input: PathBuf,
    #[arg(long)]
    pub ses: Option<PathBuf>,
    #[arg(long)]
    pub rules: Option<PathBuf>,
    /// KiCad JSON board file (legacy `-de …*.json`); not read yet — Plan 5/8 owns the loader
    #[arg(long)]
    pub kicad_json: Option<PathBuf>,
    /// Report path; stdout when omitted
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct InfoArgs {
    pub input: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_route() {
        let cli = Cli::try_parse_from([
            "freerouting",
            "route",
            "a.dsn",
            "-o",
            "b.ses",
            "--max-passes",
            "3",
            "--set",
            "x.y=1",
        ])
        .unwrap();
        match cli.command {
            Command::Route(r) => {
                assert_eq!(r.input, PathBuf::from("a.dsn"));
                assert_eq!(r.max_passes, Some(3));
                assert_eq!(r.set, vec!["x.y=1".to_string()]);
            }
            _ => panic!("expected route"),
        }
    }
    #[test]
    fn parses_kicad_json_on_route_and_drc() {
        let cli = Cli::try_parse_from([
            "freerouting",
            "route",
            "a.dsn",
            "-o",
            "b.ses",
            "--kicad-json",
            "k.json",
        ])
        .unwrap();
        match cli.command {
            Command::Route(r) => assert_eq!(r.kicad_json, Some(PathBuf::from("k.json"))),
            _ => panic!("expected route"),
        }
        let cli =
            Cli::try_parse_from(["freerouting", "drc", "a.dsn", "--kicad-json", "k.json"]).unwrap();
        match cli.command {
            Command::Drc(d) => assert_eq!(d.kicad_json, Some(PathBuf::from("k.json"))),
            _ => panic!("expected drc"),
        }
    }

    #[test]
    fn parses_mcp() {
        let cli = Cli::try_parse_from(["freerouting", "mcp"]).unwrap();
        assert!(matches!(cli.command, Command::Mcp));
    }
}
