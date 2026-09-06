use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "freerouting",
    version,
    propagate_version = true,
    about = "Headless PCB autorouter (Rust port of freerouting)"
)]
pub struct Cli {
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,
    #[arg(long, global = true)]
    pub log_level: Option<String>,
    #[arg(long, global = true, value_name = "FILE")]
    pub settings: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    Route(RouteArgs),
    Drc(DrcArgs),
    Info(InfoArgs),
    Mcp,
}

#[derive(Args, Debug)]
pub struct RouteArgs {
    pub input: PathBuf,
    #[arg(short, long)]
    pub output: PathBuf,
    #[arg(long)]
    pub rules: Option<PathBuf>,
    #[arg(long)]
    pub ses: Option<PathBuf>,
    #[arg(long)]
    pub kicad_json: Option<PathBuf>,
    #[arg(long)]
    pub kicad_project: Option<PathBuf>,
    #[arg(long)]
    pub max_passes: Option<u32>,
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
    /// Write SVG frames showing maze rooms over the routed PCB.
    #[arg(long, value_name = "DIR")]
    pub visualize: Option<PathBuf>,
    /// Capture one visualization frame for every N maze steps; route commits are always kept.
    #[arg(long, default_value_t = 1, requires = "visualize")]
    pub visualize_every: u64,
    /// Stop writing visualization frames after this many images; routing continues.
    #[arg(long, default_value_t = 1_000, requires = "visualize")]
    pub visualize_max_frames: u64,
    #[arg(long, default_value_t = 1280, requires = "visualize")]
    pub visualize_width: u32,
    #[arg(long, default_value_t = 720, requires = "visualize")]
    pub visualize_height: u32,
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
    #[arg(long)]
    pub kicad_json: Option<PathBuf>,
    #[arg(long)]
    pub kicad_project: Option<PathBuf>,
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t = DrcSchema::Kicad)]
    pub schema: DrcSchema,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DrcSchema {
    #[default]
    Kicad,
    Freerouting,
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
    fn parses_bounded_routing_visualization() {
        let cli = Cli::try_parse_from([
            "freerouting",
            "route",
            "a.dsn",
            "-o",
            "b.ses",
            "--visualize",
            "frames",
            "--visualize-every",
            "25",
            "--visualize-max-frames",
            "80",
        ])
        .unwrap();
        let Command::Route(route) = cli.command else {
            panic!("expected route");
        };
        assert_eq!(route.visualize, Some(PathBuf::from("frames")));
        assert_eq!(route.visualize_every, 25);
        assert_eq!(route.visualize_max_frames, 80);
        assert_eq!((route.visualize_width, route.visualize_height), (1280, 720));
    }

    #[test]
    fn parses_mcp() {
        let cli = Cli::try_parse_from(["freerouting", "mcp"]).unwrap();
        assert!(matches!(cli.command, Command::Mcp));
    }

    #[test]
    fn settings_is_global_and_names_a_json_file() {
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
