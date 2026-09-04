use fr_settings::prelude::{
    UNKNOWN_COMMAND_LINE_ARGUMENT_PREFIX, classify_de_arguments_reporting,
    legacy_flag_value_is_consumed,
};


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ExitCode {
        Ok = 0,
        Failure = 1,
        UsageError = 2,
                                                                                            NotImplemented = 3,
}

impl ExitCode {
        #[must_use]
    pub fn code(self) -> i32 {
        self as i32
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
        Warn,
        Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
        pub level: Level,
        pub message: String,
                    pub java_site: &'static str,
}

impl Diagnostic {
    fn warn(java_site: &'static str, message: impl Into<String>) -> Self {
        Self {
            level: Level::Warn,
            message: message.into(),
            java_site,
        }
    }

    fn error(java_site: &'static str, message: impl Into<String>) -> Self {
        Self {
            level: Level::Error,
            message: message.into(),
            java_site,
        }
    }
}

pub const BOTH_FILES_REQUIRED: &str = "Both an input file and an output file must be specified \
with command line arguments if you are running in CLI mode.";

pub const DRC_INPUT_REQUIRED: &str =
    "An input file must be specified with -de argument in DRC mode.";

pub const PARSE_PROBLEM_PREFIX: &str = "There was a problem parsing the '";
pub const PARSE_PROBLEM_SUFFIX: &str = "' parameter";

pub const DESIGN_DIRECTORY_IGNORED: &str =
    "The -di option selects a GUI input directory and is ignored: this port is headless.";

pub const COMPARE_BOARDS_IGNORED: &str =
    "The --compare-boards= option is not supported by this port and is ignored.";


#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LegacySlots {
        pub initial_input_file: Option<String>,
        pub initial_output_file: Option<String>,
        pub initial_rules_file: Option<String>,
            pub design_session_filename: Option<String>,
            pub drc_report_file: Option<String>,
        pub show_help_option: bool,
            pub stdio_mode: bool,
}

const SUBCOMMANDS: &[&str] = &["route", "drc", "info", "mcp", "help"];

#[must_use]
pub fn is_legacy_form(argv: &[String]) -> bool {
    match argv.first() {
        None => true,
        Some(first) => !SUBCOMMANDS.contains(&first.as_str()),
    }
}

#[must_use]
pub fn resolve_slots(argv: &[String]) -> (LegacySlots, Vec<Diagnostic>) {
    let mut slots = LegacySlots::default();
    let mut diagnostics: Vec<Diagnostic> = Vec::new();

    let value_of = |i: usize| -> Option<&str> {
        match argv.get(i + 1) {
            Some(next) if !next.starts_with('-') => Some(next.as_str()),
            _ => None,
        }
    };

    let mut i = 0;
    while i < argv.len() {
        let arg = argv[i].as_str();

        if arg.eq_ignore_ascii_case("-help")
            || arg.eq_ignore_ascii_case("--help")
            || arg.eq_ignore_ascii_case("-h")
        {
            slots.show_help_option = true;
            i += 1;
            continue;
        }


        if let Some(body) = arg.strip_prefix("--") {
            if body.starts_with("compare-boards=") {
                diagnostics.push(Diagnostic::warn(
                    "GlobalSettings.java:530",
                    COMPARE_BOARDS_IGNORED,
                ));
                i += 1;
                continue;
            }

            if let Some(value) = body.strip_prefix("mcp_server.stdio=")
                && (value.eq_ignore_ascii_case("true") || value == "1")
            {
                slots.stdio_mode = true;
                i += 1;
                continue;
            }

            match body.split_once('=') {
                Some(("user_data_path", _)) => {}
                Some(_) => {}
                None if body == "user_data_path" => {}
                None => diagnostics.push(Diagnostic::warn(
                    "GlobalSettings.java:562",
                    format!("{UNKNOWN_COMMAND_LINE_ARGUMENT_PREFIX}{arg}"),
                )),
            }
            i += 1;
            continue;
        }

        if arg.starts_with("-de") {
            if value_of(i).is_some() {
                let mut j = i + 1;
                while j < argv.len() && !argv[j].starts_with('-') {
                    j += 1;
                }
                let (de, warnings) = classify_de_arguments_reporting(&argv[i..j]);
                for warning in warnings {
                    diagnostics.push(Diagnostic::warn(de_warning_site(&warning), warning));
                }
                if de.initial_input_file.is_some() {
                    slots.initial_input_file = de.initial_input_file;
                }
                if de.design_session_filename.is_some() {
                    slots.design_session_filename = de.design_session_filename;
                }
                if de.initial_rules_file.is_some() {
                    slots.initial_rules_file = de.initial_rules_file;
                }
                i = j;
                continue;
            }
        } else if arg.starts_with("-di") {
            if value_of(i).is_some() {
                diagnostics.push(Diagnostic::warn(
                    "GlobalSettings.java:651",
                    DESIGN_DIRECTORY_IGNORED,
                ));
                i += 1;
            }
        } else if arg.starts_with("-do") {
            if let Some(value) = value_of(i) {
                slots.initial_output_file = Some(value.to_string());
                i += 1;
            }
        } else if arg.starts_with("-drc") {
            if let Some(value) = value_of(i) {
                slots.drc_report_file = Some(value.to_string());
                i += 1;
            }
        } else if arg.starts_with("-dr") {
            if let Some(value) = value_of(i) {
                slots.initial_rules_file = Some(value.to_string());
                i += 1;
            }
        } else if arg.starts_with("-mp")
            || arg.starts_with("-mt")
            || arg.starts_with("-oit")
            || arg.starts_with("-us")
            || arg.starts_with("-is")
            || arg.starts_with("-hr")
        {
            if let Some(value) = value_of(i) {
                if legacy_flag_value_is_consumed(arg, value) {
                    i += 1;
                } else {
                    diagnostics.push(Diagnostic::error(
                        "GlobalSettings.java:836",
                        format!("{PARSE_PROBLEM_PREFIX}{arg}{PARSE_PROBLEM_SUFFIX}"),
                    ));
                }
            }
        } else if arg == "-l" {
            if value_of(i).is_some() {
                i += 1;
            }
        } else if arg.starts_with("-dl") || arg.starts_with("-da") {
        } else if arg.starts_with("-host") {
            if value_of(i).is_some() {
                i += 1;
            }
        } else if arg.starts_with("-help") {
            slots.show_help_option = true;
        } else if arg.starts_with("-inc") || arg.starts_with("-dct") || arg.starts_with("-ll") {
            if let Some(value) = value_of(i) {
                if legacy_flag_value_is_consumed(arg, value) {
                    i += 1;
                } else {
                    diagnostics.push(Diagnostic::error(
                        "GlobalSettings.java:836",
                        format!("{PARSE_PROBLEM_PREFIX}{arg}{PARSE_PROBLEM_SUFFIX}"),
                    ));
                }
            }
        } else {
            diagnostics.push(Diagnostic::warn(
                "GlobalSettings.java:833",
                format!("{UNKNOWN_COMMAND_LINE_ARGUMENT_PREFIX}{arg}"),
            ));
        }

        i += 1;
    }

    (slots, diagnostics)
}

#[must_use]
pub fn rewrite(argv: &[String]) -> (Vec<String>, Vec<Diagnostic>) {
    if !is_legacy_form(argv) {
        return (argv.to_vec(), Vec::new());
    }

    let (slots, mut diagnostics) = resolve_slots(argv);

    if slots.show_help_option {
        return (vec!["--help".to_string()], diagnostics);
    }
    if let Some(report) = slots.drc_report_file {
        let Some(input) = slots.initial_input_file else {
            diagnostics.push(Diagnostic::error(
                "Freerouting.java:248",
                DRC_INPUT_REQUIRED,
            ));
            return (Vec::new(), diagnostics);
        };
        let mut out = vec!["drc".to_string(), input];
        push_option(&mut out, "--ses", slots.design_session_filename);
        push_option(&mut out, "--rules", slots.initial_rules_file);
        out.push("-o".to_string());
        out.push(report);
        return (out, diagnostics);
    }

    if slots.stdio_mode {
        return (vec!["mcp".to_string()], diagnostics);
    }

    let (Some(input), Some(output)) = (slots.initial_input_file, slots.initial_output_file) else {
        diagnostics.push(Diagnostic::error(
            "Freerouting.java:81",
            BOTH_FILES_REQUIRED,
        ));
        return (Vec::new(), diagnostics);
    };
    let mut out = vec!["route".to_string(), input];
    push_option(&mut out, "--ses", slots.design_session_filename);
    push_option(&mut out, "--rules", slots.initial_rules_file);
    out.push("-o".to_string());
    out.push(output);
    (out, diagnostics)
}

fn push_option(out: &mut Vec<String>, flag: &str, value: Option<String>) {
    if let Some(value) = value {
        out.push(flag.to_string());
        out.push(value);
    }
}

fn de_warning_site(warning: &str) -> &'static str {
    use fr_settings::prelude::{
        MULTIPLE_DSN_FILES, MULTIPLE_RULES_FILES, MULTIPLE_SES_FILES, MULTIPLE_SESSION_FILES,
    };
    if warning == MULTIPLE_DSN_FILES {
        "GlobalSettings.java:602"
    } else if warning == MULTIPLE_SESSION_FILES {
        "GlobalSettings.java:615"
    } else if warning == MULTIPLE_SES_FILES {
        "GlobalSettings.java:623"
    } else if warning == MULTIPLE_RULES_FILES {
        "GlobalSettings.java:631"
    } else {
        "GlobalSettings.java:638"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| (*x).to_string()).collect()
    }

    fn argv_of(v: &[&str]) -> Vec<String> {
        rewrite(&s(v)).0
    }

    fn warnings_of(v: &[&str]) -> Vec<String> {
        rewrite(&s(v)).1.into_iter().map(|d| d.message).collect()
    }

    #[test]
    fn a_native_command_line_passes_through_untouched() {
        for native in [
            s(&["route", "a.dsn", "-o", "b.ses"]),
            s(&["drc", "a.dsn"]),
            s(&["info", "a.dsn"]),
            s(&["mcp"]),
        ] {
            let (out, diagnostics) = rewrite(&native);
            assert_eq!(out, native);
            assert!(diagnostics.is_empty());
        }
    }

    #[test]
    fn de_and_do_become_route() {
        assert_eq!(
            argv_of(&["-de", "a.dsn", "-do", "b.ses"]),
            s(&["route", "a.dsn", "-o", "b.ses"])
        );
    }

    #[test]
    fn every_short_flag_but_l_is_prefix_matched() {
        assert_eq!(
            argv_of(&["-decoy", "a.dsn", "-do", "b.ses"]),
            s(&["route", "a.dsn", "-o", "b.ses"])
        );
        assert_eq!(
            argv_of(&["-de", "a.dsn", "-drcx", "r.json"]),
            s(&["drc", "a.dsn", "-o", "r.json"])
        );
        assert_eq!(
            argv_of(&["-de", "a.dsn", "-dox", "b.ses"]),
            s(&["route", "a.dsn", "-o", "b.ses"])
        );
    }

    #[test]
    fn a_missing_value_is_a_silent_no_op_and_i_is_not_advanced() {
        let (out, diagnostics) = rewrite(&s(&["-de", "a.dsn", "-do", "b.ses", "-mp", "-5"]));
        assert_eq!(out, s(&["route", "a.dsn", "-o", "b.ses"]));
        assert_eq!(
            diagnostics
                .iter()
                .map(|d| d.message.as_str())
                .collect::<Vec<_>>(),
            vec!["Unknown command line argument: -5"]
        );
    }

    #[test]
    fn an_unknown_flag_warns_and_continues() {
        let (out, diagnostics) = rewrite(&s(&["-de", "a.dsn", "-zz", "-do", "b.ses"]));
        assert_eq!(out, s(&["route", "a.dsn", "-o", "b.ses"]));
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].level, Level::Warn);
        assert_eq!(diagnostics[0].message, "Unknown command line argument: -zz");
    }

    #[test]
    fn ruling_14_json_takes_javas_slot() {
        assert_eq!(
            argv_of(&["-de", "board.json", "-do", "b.ses"]),
            s(&["route", "board.json", "-o", "b.ses"])
        );
        assert_eq!(
            argv_of(&["-de", "a.dsn", "prev.json", "-do", "b.ses"]),
            s(&["route", "a.dsn", "--ses", "prev.json", "-o", "b.ses"])
        );
    }

    #[test]
    fn a_bare_drc_is_not_drc_mode() {
        let (out, diagnostics) = rewrite(&s(&["-de", "a.dsn", "-drc"]));
        assert!(out.is_empty());
        assert_eq!(
            diagnostics.last().expect("one error").message,
            BOTH_FILES_REQUIRED
        );
    }

    #[test]
    fn drc_without_an_input_is_javas_other_refusal() {
        let (out, diagnostics) = rewrite(&s(&["-drc", "r.json"]));
        assert!(out.is_empty());
        assert_eq!(
            diagnostics.last().expect("one error").message,
            DRC_INPUT_REQUIRED
        );
    }

    #[test]
    fn no_arguments_at_all_is_javas_cli_refusal() {
        let (out, diagnostics) = rewrite(&[]);
        assert!(out.is_empty());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].level, Level::Error);
        assert_eq!(diagnostics[0].message, BOTH_FILES_REQUIRED);
    }

    #[test]
    fn dr_overwrites_the_rules_slot_de_filled() {
        assert_eq!(
            argv_of(&["-de", "a.dsn", "x.rules", "-do", "b.ses", "-dr", "y.rules"]),
            s(&["route", "a.dsn", "--rules", "y.rules", "-o", "b.ses"])
        );
    }

    #[test]
    fn the_de_warnings_come_out_in_javas_order() {
        assert_eq!(
            warnings_of(&["-zz", "-de", "a.dsn", "b.dsn", "junk.bin", "-do", "o.ses"]),
            vec![
                "Unknown command line argument: -zz".to_string(),
                "Multiple DSN files provided in -de argument. Only the last one will be used."
                    .to_string(),
                "Unknown file type in -de argument: junk.bin. Expected .dsn, .json, .ses, or .rules"
                    .to_string(),
            ]
        );
    }

    #[test]
    fn help_wins_over_everything() {
        assert_eq!(
            argv_of(&["-de", "a.dsn", "-do", "b.ses", "-h"]),
            s(&["--help"])
        );
        assert_eq!(argv_of(&["--help"]), s(&["--help"]));
        assert_eq!(argv_of(&["-HELP"]), s(&["--help"]));
    }

    #[test]
    fn version_is_an_unknown_argument_on_the_legacy_path() {
        let (out, diagnostics) = rewrite(&s(&["--version"]));
        assert!(out.is_empty());
        assert_eq!(
            diagnostics
                .iter()
                .map(|d| d.message.as_str())
                .collect::<Vec<_>>(),
            vec![
                "Unknown command line argument: --version",
                BOTH_FILES_REQUIRED,
            ]
        );
        let (out, diagnostics) = rewrite(&s(&["-V"]));
        assert!(out.is_empty());
        assert_eq!(diagnostics[0].message, "Unknown command line argument: -V");
        assert_eq!(
            argv_of(&["-de", "a.dsn", "-do", "b.ses", "--version"]),
            s(&["route", "a.dsn", "-o", "b.ses"])
        );
    }

    #[test]
    fn the_mcp_stdio_long_form_becomes_the_mcp_subcommand() {
        assert_eq!(argv_of(&["--mcp_server.stdio=true"]), s(&["mcp"]));
        assert_eq!(argv_of(&["--mcp_server.stdio=1"]), s(&["mcp"]));
        let (out, _) = rewrite(&s(&["--mcp_server.stdio=no"]));
        assert!(out.is_empty());
    }

    #[test]
    fn di_consumes_its_value_and_warns() {
        let (out, diagnostics) = rewrite(&s(&["-di", "dir", "-de", "a.dsn", "-do", "b.ses"]));
        assert_eq!(out, s(&["route", "a.dsn", "-o", "b.ses"]));
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].message, DESIGN_DIRECTORY_IGNORED);
    }

    #[test]
    fn the_valued_flags_consume_their_values_and_forward_nothing() {
        assert_eq!(
            argv_of(&[
                "-de", "a.dsn", "-do", "b.ses", "-mp", "5", "-mt", "2", "-oit", "50", "-us",
                "hybrid", "-is", "random", "-hr", "1:1", "-inc", "GND,VCC", "-l", "de", "-host",
                "x", "-dct", "5", "-ll", "debug", "-dl", "-da",
            ]),
            s(&["route", "a.dsn", "-o", "b.ses"])
        );
    }

    #[test]
    fn a_malformed_number_logs_and_leaves_its_value_to_be_re_read() {
        let (out, diagnostics) = rewrite(&s(&["-de", "a.dsn", "-do", "b.ses", "-mp", "abc"]));
        assert_eq!(out, s(&["route", "a.dsn", "-o", "b.ses"]));
        assert_eq!(
            diagnostics
                .iter()
                .map(|d| (d.level, d.message.as_str()))
                .collect::<Vec<_>>(),
            vec![
                (
                    Level::Error,
                    "There was a problem parsing the '-mp' parameter"
                ),
                (Level::Warn, "Unknown command line argument: abc"),
            ]
        );
        let (_, diagnostics) = rewrite(&s(&["-de", "a.dsn", "-do", "b.ses", "-dct", "0x10"]));
        assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
        let (_, diagnostics) = rewrite(&s(&["-de", "a.dsn", "-do", "b.ses", "-mp", "0x10"]));
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn exit_codes_are_the_ruling_ar_ladder() {
        assert_eq!(ExitCode::Ok.code(), 0);
        assert_eq!(ExitCode::Failure.code(), 1);
        assert_eq!(ExitCode::UsageError.code(), 2);
        assert_eq!(ExitCode::NotImplemented.code(), 3);
    }

                                    #[test]
    fn no_command_runner_answers_not_implemented() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut offenders = Vec::new();
        let mut stack = vec![src];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).expect("the crate's own src/ is readable") {
                let path = entry.expect("a readable directory entry").path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().is_some_and(|e| e == "rs") {
                    let text = std::fs::read_to_string(&path).expect("a readable source file");
                    if path.file_name().is_some_and(|n| n == "legacy.rs") {
                        continue;
                    }
                    for (n, line) in text.lines().enumerate() {
                        if line.contains("ExitCode::NotImplemented") {
                            offenders.push(format!("{}:{}", path.display(), n + 1));
                        }
                    }
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "ExitCode::NotImplemented is reserved and unreachable; these sites answer it: {offenders:?}"
        );
    }
}
