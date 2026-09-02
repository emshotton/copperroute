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
//! disappointed, so each port-only option is marked here and in `crates/freerouting/README.md`,
//! whose "Port-only spellings" table is the single list. `--version` is the sharpest case: the jar
//! **refuses** it (`Unknown command line argument: --version`, then exit 1 — measured), so
//! controller ruling BF confines it to the native form, and `crate::legacy` has no arm for it.

use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

/// `propagate_version` is load-bearing, not cosmetic: controller ruling BF put `--version` on the
/// **native** subcommand form only, because on the legacy form the jar answers it with
/// `Unknown command line argument: --version` and **exit 1** (measured — `p8t5`'s `version-long`
/// and `version-short` rows, and the jar run recorded in the Task 5 report). `crate::legacy` has
/// no `--version` arm at all, so the only way to reach clap's is behind a subcommand, and that
/// only works with the version flag propagated: `freerouting route --version`.
#[derive(Parser, Debug)]
#[command(
    name = "freerouting",
    version,
    propagate_version = true,
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
    /// (`settings/sources/JsonFileSettings.java`). Without it there is **no** priority-10 source
    /// at all. Port only, and **this subcommand form only**: Java has no flag for this, and on
    /// the legacy command line the port warns exactly as the jar does and applies nothing
    /// (rulings R7 and BG).
    ///
    /// **Wired in Plan 8 Task 6**, which added `SettingsInputs::json_file` and made
    /// `commands::route` fill it from `JsonFileSettings::new`. The Task 5 `// obligation:` that
    /// stood here, and the ~~"NOT YET WIRED (Plan 8 Task 6)"~~ opening of this help text, are
    /// discharged.
    ///
    /// The ~~"Without it the working directory's `freerouting.json` is used if it exists — the
    /// port's stand-in for Java's OS-standard user-data path"~~ sentence stood here through Task 6
    /// round 1 and **controller ruling BG removed it**: measured at the pinned jar, a
    /// `freerouting.json` in the working directory changes nothing (the jar reads
    /// `GlobalSettings.getUserDataPath()` and only that), so the stand-in stood in for no jar
    /// behaviour and was a port-only default a stray file could have used to change a routing
    /// result silently.
    ///
    /// Note that `commands::route` recovers the value from the **raw** argv rather than from this
    /// field, because the runners are handed the raw argv and not the parsed `Cli` (see
    /// [`crate::run`]); this declaration is what puts the flag in `--help` and what makes clap
    /// accept it.
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
    /// Accepted and **inert on every path** — controller ruling AQ and answer 4, quirk #143.
    /// Java's `-mt` writes `optimizer.max_threads`, whose every reader is GUI-only or dead code:
    /// the headless chain is `RoutingPipeline.createForHeadless:45-47` ->
    /// `BatchOptimizer.createForHeadless:51-53`, which returns the single-threaded optimizer
    /// unconditionally, and `BatchAutorouter.autoroutePassMultiThread:411-413` has no caller
    /// anywhere in the tree. The port reproduces that: the value is parsed, merged and reported,
    /// and nothing routes with it.
    ///
    /// **It does not control the MCP server's threads either**, and that is the deliberate part.
    /// The port's only two `std::thread` spawn sites are both under `crates/freerouting/src/mcp/`
    /// (ruling 3), and they are a *transport* — one reader for stdin, one per in-flight
    /// `tools/call` so a long route can be cancelled — not a routing policy. Wiring `--threads`
    /// to them would resurrect a flag the jar reads nowhere and would tie an operator's routing
    /// knob to a protocol detail.
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
    /// Generic settings override, `section.field=value` (repeatable). **This is the native form's
    /// only generic override**; on the legacy form the spelling is Java's own
    /// `--section.field=value`, and neither command line takes the other's.
    ///
    /// # The two spellings, and why there are two (controller ruling BJ)
    ///
    /// | form | spelling | example |
    /// |---|---|---|
    /// | native (this one) | `--set <section>.<field>=<value>` | `freerouting route b.dsn -o b.ses --set router.max_passes=7` |
    /// | legacy | `--<section>.<field>=<value>` — Java's own | `freerouting -de b.dsn -do b.ses --router.max_passes=7` |
    ///
    /// `clap` owns the native command line and has **no arm** for a free-form
    /// `--<section>.<field>=<value>`: `freerouting route b.dsn -o b.ses
    /// --router.optimizer.max_threads=4` answers `error: unexpected argument
    /// '--router.optimizer.max_threads' found` and exits **2**. And the legacy path is
    /// bug-for-bug (ruling AR), where the jar ignores `--set` twice over — no `=` on the flag,
    /// and its payload does not start with `-`, so neither branch of
    /// `CliSettings.java`'s loop sees either token. So the port cannot offer one spelling on both
    /// forms without either breaking `clap` or diverging from the jar, and it offers each form the
    /// one that fits it.
    ///
    /// Beyond the spelling the two are **the same code path**:
    /// `fr_settings::CliSettings::new_with_set_alias` splits the payload at its first `=`, ignores
    /// a name that does not start with `router.`, and hands the rest to the very
    /// `apply_router_setting` the direct spelling reaches — priority 60, `ReflectionUtil
    /// .setFieldValue`'s semantics, warn-and-skip on a bad value.
    /// `crates/freerouting/src/commands::cli_settings` is the one place that chooses between the
    /// two, on `legacy::is_legacy_form`.
    ///
    /// This is how to reach a setting the five **deliberately dead** legacy flags only pretend to
    /// write (quirk #131, ruling AQ): `--set
    /// router.optimizer.optimization_improvement_threshold=0.005` is what `-oit 5` looks like it
    /// should do and does not.
    ///
    /// **`--set router.max_items=N` also stops the optimizer; `--max-passes` does not**
    /// (quirk #202). The two limits take different arms of Java's three-state stop flag:
    /// `AutoroutePassRunner.java:212-221` reaches `max_items` and calls `requestStop()`, which
    /// writes `ALL`, and `RoutingPipeline.runOptimizationStage` returns early on `ALL` — so the
    /// board that reaches the SES is **unoptimised**. `AutorouteBatchLoop.java:267-271` reaches
    /// `max_passes` and calls `requestStopAutoRouter()`, which writes `AUTO_ROUTER_ONLY`, and the
    /// optimizer runs normally. Java says none of this: the message it logs is "Max items limit
    /// reached. Stopping auto-router.", and the optimizer is not the auto-router. The port
    /// reproduces the behaviour exactly and states it here, which is the discharge Plan 7's
    /// hand-off asked for. The port has no `--max-items` flag of its own, so `--set
    /// router.max_items=N` (native) and `--router.max_items=N` (legacy) are the whole surface
    /// for it.
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
    /// Which spelling of the KiCad DRC schema the report uses (see [`DrcSchema`]). Port only:
    /// Java has one spelling per jar and no flag at all.
    #[arg(long, value_enum, default_value_t = DrcSchema::Kicad)]
    pub schema: DrcSchema,
}

/// The two `@SerializedName` spellings of the KiCad DRC report — quirk #154, and **ruling W's
/// flag**.
///
/// The report's first key is `"$schema": "https://schemas.kicad.org/drc.v1.json"`, and that
/// schema is snake_case: KiCad 9.0.1 and freerouting **2.3.0** both write `coordinate_units` /
/// `unconnected_items` / `quality_score`, and the clone's HEAD writes `coordinateUnits` /
/// `unconnectedItems` / `qualityScore` instead. So the jar advertises a schema it does not
/// validate against, and a consumer written for KiCad breaks on it.
///
/// `fr_drc::DrcJsonFlavor`'s `Default` stays `FreeroutingHead`, because that is the **parity**
/// choice `crates/fr-drc/tests/report_json.rs` pins against the jar's own Gson bytes. **The CLI's
/// default is the other one** (ruling W): [`DrcSchema::Kicad`], the spelling `$schema` promises,
/// passed *explicitly* at the call site in `commands::drc` so that a future change to the enum's
/// `Default` cannot silently move the CLI. [`DrcSchema::Freerouting`] is the way back to HEAD's
/// bytes, and it is what `scripts/differential/rust/src/bin/p8t3.rs` runs the port with — a
/// byte-for-byte comparison against the jar needs the jar's spelling.
#[derive(clap::ValueEnum, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DrcSchema {
    /// KiCad's own spelling, which is also freerouting 2.3.0's — `coordinate_units`,
    /// `unconnected_items`, `quality_score`, and the violation types `hole_clearance` /
    /// `unconnected_items`. The CLI default (ruling W).
    #[default]
    Kicad,
    /// The clone's HEAD spelling — camelCase keys and the violation types `holeClearance` /
    /// `unconnectedItems`. Bug-compatible with the jar, which is why the differential driver
    /// asks for it.
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
