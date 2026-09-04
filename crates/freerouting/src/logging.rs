//! | `File` | the resolved log file (`:76-84`) | the file level, default `DEBUG` (`:107-109`) |
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum LogLevel {
        Off,
        Error,
        Warn,
        #[default]
    Info,
    /// `Level.DEBUG`.
    Debug,
        Trace,
}

impl LogLevel {
                /// `Level.valueOf` knows eight names — `OFF`, `FATAL`, `ERROR`, `WARN`, `INFO`, `DEBUG`,
                        #[must_use]
    pub fn parse_java(level: &str) -> Self {
        match level.to_uppercase().as_str() {
            "OFF" => Self::Off,
            "FATAL" | "ERROR" => Self::Error,
            "WARN" => Self::Warn,
            "DEBUG" => Self::Debug,
            "TRACE" | "ALL" => Self::Trace,
            _ => Self::Info,
        }
    }

        #[must_use]
    pub fn as_filter(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        }
    }
}

#[must_use]
pub fn console_level_string(argv: &[String]) -> String {
    let mut level = "INFO".to_string();

    let mut i = 0;
    while i < argv.len() {
        let arg = argv[i].as_str();
        if let Some(value) = arg.strip_prefix("--logging.console.level=") {
            level = value.to_string();
        } else if !arg.starts_with("--") && arg.starts_with("-ll") {
            match argv.get(i + 1) {
                Some(next) if !next.starts_with('-') => {
                    level = next.to_uppercase();
                    i += 1;
                }
                _ => {}
            }
        }
        i += 1;
    }

    level
}

#[must_use]
pub fn level_from_argv(argv: &[String]) -> LogLevel {
    let mut level = LogLevel::parse_java(&console_level_string(argv));
    let mut verbosity = 0u8;

    let mut i = 0;
    while i < argv.len() {
        let arg = argv[i].as_str();
        if arg == "--log-level" {
            if let Some(value) = argv.get(i + 1) {
                level = LogLevel::parse_java(value);
                i += 1;
            }
        } else if let Some(value) = arg.strip_prefix("--log-level=") {
            level = LogLevel::parse_java(value);
        } else if arg == "-v" || arg == "--verbose" {
            verbosity = verbosity.saturating_add(1);
        } else if arg == "-vv" {
            verbosity = verbosity.saturating_add(2);
        }
        i += 1;
    }

    match verbosity {
        0 => level,
        1 => level.max(LogLevel::Debug),
        _ => LogLevel::Trace,
    }
}

static INITIALISED: AtomicBool = AtomicBool::new(false);

pub fn init(level: LogLevel) {
    if INITIALISED.swap(true, Ordering::SeqCst) {
        return;
    }
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(level.as_filter()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_target(false)
        .without_time()
        .init();
}

pub const MESSAGE_MAP: &[(&str, &str)] = &[
    (
        "GlobalSettings.java:562",
        "Unknown command line argument: {}",
    ),
    (
        "GlobalSettings.java:602",
        "Multiple DSN files provided in -de argument. Only the last one will be used.",
    ),
    (
        "GlobalSettings.java:615",
        "Multiple session files (SES/JSON) provided in -de argument. Only the last one will be used.",
    ),
    (
        "GlobalSettings.java:623",
        "Multiple SES files provided in -de argument. Only the last one will be used.",
    ),
    (
        "GlobalSettings.java:631",
        "Multiple RULES files provided in -de argument. Only the last one will be used.",
    ),
    (
        "GlobalSettings.java:638",
        "Unknown file type in -de argument: {}. Expected .dsn, .json, .ses, or .rules",
    ),
    (
        "GlobalSettings.java:833",
        "Unknown command line argument: {}",
    ),
    (
        "GlobalSettings.java:836",
        "There was a problem parsing the '{}' parameter",
    ),
    ("GlobalSettings.java:503", "Unknown settings property: {}"),
    (
        "GlobalSettings.java:506",
        "Failed to set property value for: {}",
    ),
    (
        "Freerouting.java:81",
        "Both an input file and an output file must be specified with command line arguments if you are running in CLI mode.",
    ),
    ("Freerouting.java:105", "Couldn't load the input file '{}'"),
    (
        "Freerouting.java:109",
        "Couldn't read the input file '{}', aborting.",
    ),
    ("Freerouting.java:119", "Couldn't delete the file '{}'"),
    ("Freerouting.java:138", "Couldn't load rules file '{}': {}"),
    ("Freerouting.java:210", "Couldn't save the output file '{}'"),
    (
        "Freerouting.java:238",
        "Couldn't write routing result manifest to '{}'",
    ),
    (
        "Freerouting.java:248",
        "An input file must be specified with -de argument in DRC mode.",
    ),
    ("Freerouting.java:263", "Loading DSN file for DRC: {}"),
    ("Freerouting.java:272", "Failed to load board for DRC check"),
    ("Freerouting.java:281", "Loading RULES file for DRC: {}"),
    (
        "Freerouting.java:286",
        "RULES file loaded for DRC successfully",
    ),
    ("Freerouting.java:289", "RULES file for DRC not found: {}"),
    ("Freerouting.java:292", "Failed to load RULES file for DRC"),
    (
        "Freerouting.java:303",
        "Loading KiCad JSON session file for DRC: {}",
    ),
    (
        "Freerouting.java:306",
        "KiCad JSON session file loaded for DRC successfully",
    ),
    ("Freerouting.java:309", "Loading SES file for DRC: {}"),
    (
        "Freerouting.java:312",
        "SES file loaded for DRC: {} wires, {} vias imported{}",
    ),
    ("Freerouting.java:324", "Session file for DRC not found: {}"),
    (
        "Freerouting.java:327",
        "Failed to load session file for DRC",
    ),
    (
        "Freerouting.java:351",
        "Failed to calculate quality score for DRC report: {}",
    ),
    ("Freerouting.java:363", "DRC report written to: {}"),
    (
        "Freerouting.java:365",
        "Couldn't save the DRC report to '{}'",
    ),
    ("Freerouting.java:1120", "Freerouting {}"),
    ("Freerouting.java:1450", "Couldn't initialize the GUI"),
];


#[cfg(test)]
mod tests {
    use super::*;

    fn argv(args: &[&str]) -> Vec<String> {
        args.iter().map(|a| (*a).to_string()).collect()
    }

    #[test]
    fn the_default_console_level_is_java_s_info() {
        assert_eq!(level_from_argv(&argv(&[])), LogLevel::Info);
        assert_eq!(level_from_argv(&argv(&["route", "a.dsn"])), LogLevel::Info);
    }

    #[test]
    fn parse_java_falls_back_to_info_and_never_fails() {
        assert_eq!(LogLevel::parse_java("debug"), LogLevel::Debug);
        assert_eq!(LogLevel::parse_java("DeBuG"), LogLevel::Debug);
        assert_eq!(LogLevel::parse_java("fatal"), LogLevel::Error);
        assert_eq!(LogLevel::parse_java("all"), LogLevel::Trace);
        assert_eq!(LogLevel::parse_java("off"), LogLevel::Off);
        assert_eq!(LogLevel::parse_java("verbose"), LogLevel::Info);
        assert_eq!(LogLevel::parse_java(""), LogLevel::Info);
    }

    #[test]
    fn ll_is_prefix_matched_and_last_wins() {
        assert_eq!(level_from_argv(&argv(&["-lll", "debug"])), LogLevel::Debug);
        assert_eq!(
            level_from_argv(&argv(&["-ll", "trace", "-ll", "error"])),
            LogLevel::Error
        );
    }

    #[test]
    fn ll_without_a_value_is_a_silent_no_op() {
        assert_eq!(level_from_argv(&argv(&["-ll"])), LogLevel::Info);
        assert_eq!(level_from_argv(&argv(&["-ll", "-de"])), LogLevel::Info);
    }

    #[test]
    fn the_console_level_string_is_javas_field_verbatim() {
        assert_eq!(console_level_string(&argv(&[])), "INFO");
        assert_eq!(console_level_string(&argv(&["-ll", "debug"])), "DEBUG");
        assert_eq!(
            console_level_string(&argv(&["--logging.console.level=debug"])),
            "debug"
        );
        assert_eq!(console_level_string(&argv(&["-ll", "garbage"])), "GARBAGE");
        assert_eq!(LogLevel::parse_java("GARBAGE"), LogLevel::Info);
    }

    #[test]
    fn the_port_only_spellings_are_accepted_too() {
        assert_eq!(
            level_from_argv(&argv(&["--logging.console.level=warn"])),
            LogLevel::Warn
        );
        assert_eq!(
            level_from_argv(&argv(&["--log-level", "trace"])),
            LogLevel::Trace
        );
        assert_eq!(level_from_argv(&argv(&["-v"])), LogLevel::Debug);
        assert_eq!(level_from_argv(&argv(&["-vv"])), LogLevel::Trace);
    }

    #[test]
    fn the_message_map_is_unique_by_call_site_and_never_empty() {
        let mut seen = std::collections::BTreeSet::new();
        for (site, message) in MESSAGE_MAP {
            assert!(
                seen.insert(*site),
                "duplicate FRLogger call site in MESSAGE_MAP: {site}"
            );
            assert!(!message.is_empty(), "empty message for {site}");
            assert!(
                site.contains(".java:"),
                "MESSAGE_MAP keys are <file>:<line>, got {site}"
            );
        }
    }
}
