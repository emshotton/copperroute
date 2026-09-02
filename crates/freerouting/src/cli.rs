//! The **native** subcommand form — the port's own command line, which Java does not have.
//!
//! Java has one command line and one parser table (`GlobalSettings.applyCommandLineArguments`,
//! `:521-838`); `crate::legacy` reproduces it and rewrites it into the form declared here. This
//! module is therefore **not a port of anything**: every option below is either a re-spelling of a
//! legacy flag or a port-only addition, and the ones that are port-only say so.
//!
//! Quirk #260 is the reason that matters. Java's help text documents ten of its twenty-four
//! accepted flags, and **there is no `-v`/`--verbose`** — the log-level flag is `-ll`. A reader
//! who learns this CLI from `--help` and then types the same thing at the jar will be
//! disappointed, so each port-only option is marked here and in `crates/freerouting/README.md`.

use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "freerouting",
    version,
    about = "Headless PCB autorouter (Rust port of freerouting)"
)]
pub struct Cli {
    /// Increase log verbosity (-v, -vv). Port only: Java has no -v (quirk #260); its log-level
    /// flag is `-ll <level>`, which the legacy form still accepts.
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,
    /// Explicit log level (off|error|warn|info|debug|trace); overrides -v. The port's spelling of
    /// Java's `-ll` / `--logging.console.level=`; unrecognised names fall back to `info`, exactly
    /// as `Log4j2ConfigurationFactory.parseLevel` (:130-135) does.
    #[arg(long, global = true)]
    pub log_level: Option<String>,
    /// A `freerouting.json` to read router settings from, at priority 10
    /// (`settings/sources/JsonFileSettings.java`). Without it the working directory's
    /// `freerouting.json` is used if it exists — the port's stand-in for Java's OS-standard
    /// user-data path, which is not ported (spec §2). Port only: Java has no flag for this.
    #[arg(long, global = true, value_name = "FILE")]
    pub settings: Option<PathBuf>,
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
    /// KiCad JSON board file. **Native form only** (ruling 14): on the legacy form a `.json` in
    /// `-de` follows Java's own slot rule — design input while no `.dsn` has been seen, session
    /// afterwards (`GlobalSettings.java:609-621`) — and never reaches this option.
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
    /// KiCad JSON board file. **Native form only** (ruling 14) — see `RouteArgs::kicad_json`.
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

    #[test]
    fn settings_is_global_and_names_a_json_file() {
        // Scan ruling R7: `--settings <file>` is `JsonFileSettings(Path)`'s constructor.
        let cli = Cli::try_parse_from([
            "freerouting",
            "route",
            "a.dsn",
            "-o",
            "b.ses",
            "--settings",
            "s.json",
        ])
        .unwrap();
        assert_eq!(cli.settings, Some(PathBuf::from("s.json")));
        let cli =
            Cli::try_parse_from(["freerouting", "--settings", "s.json", "info", "a.dsn"]).unwrap();
        assert_eq!(cli.settings, Some(PathBuf::from("s.json")));
    }
}
