//! Rewrites Java-freerouting command lines (`-de in.dsn -do out.ses -mp 100 …`)
//! into the subcommand form (`route in.dsn -o out.ses --max-passes 100`).

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LegacyError {
    #[error("legacy flag {0} is not supported by freerouting-rs")]
    Unsupported(String),
    #[error("legacy flag {0} requires a value")]
    MissingValue(String),
    #[error("-de given without -do or -drc")]
    MissingOutput,
    #[error("-de input must include a .dsn file")]
    MissingDsn,
}

const LEGACY_TRIGGERS: &[&str] = &[
    "-de", "-do", "-drc", "-di", "-dr", "-mp", "-mt", "-oit", "-inc", "-us", "-hr", "-is",
];

/// Returns `argv` unchanged if it contains no legacy flags.
pub fn rewrite(argv: &[String]) -> Result<Vec<String>, LegacyError> {
    let has_legacy = argv
        .iter()
        .any(|a| LEGACY_TRIGGERS.contains(&a.as_str()) || is_generic_override(a));
    if !has_legacy {
        return Ok(argv.to_vec());
    }

    let mut dsn: Option<String> = None;
    let mut ses: Option<String> = None;
    let mut rules: Option<String> = None;
    let mut output: Option<String> = None;
    let mut drc_output: Option<String> = None;
    let mut extra: Vec<String> = Vec::new();

    let mut i = 0;
    let take_value = |i: &mut usize, flag: &str| -> Result<String, LegacyError> {
        *i += 1;
        argv.get(*i)
            .cloned()
            .ok_or_else(|| LegacyError::MissingValue(flag.to_string()))
    };
    while i < argv.len() {
        let a = argv[i].as_str();
        match a {
            "-de" => {
                let v = take_value(&mut i, a)?;
                for part in v.split('+') {
                    let lower = part.to_ascii_lowercase();
                    if lower.ends_with(".dsn") {
                        dsn = Some(part.to_string());
                    } else if lower.ends_with(".ses") {
                        ses = Some(part.to_string());
                    } else if lower.ends_with(".rules") {
                        rules = Some(part.to_string());
                    } else {
                        dsn = Some(part.to_string());
                    }
                }
            }
            "-do" => output = Some(take_value(&mut i, a)?),
            "-drc" => drc_output = Some(take_value(&mut i, a)?),
            "-dr" => rules = Some(take_value(&mut i, a)?),
            "-di" => return Err(LegacyError::Unsupported(a.to_string())),
            "-mp" => push_pair(&mut extra, "--max-passes", take_value(&mut i, a)?),
            "-mt" => push_pair(&mut extra, "--threads", take_value(&mut i, a)?),
            "-oit" => push_pair(
                &mut extra,
                "--optimizer-improvement-threshold",
                take_value(&mut i, a)?,
            ),
            "-inc" => push_pair(&mut extra, "--ignore-net-classes", take_value(&mut i, a)?),
            "-us" => push_pair(&mut extra, "--update-strategy", take_value(&mut i, a)?),
            "-hr" => push_pair(&mut extra, "--hybrid-ratio", take_value(&mut i, a)?),
            "-is" => push_pair(&mut extra, "--item-selection", take_value(&mut i, a)?),
            // Value-taking flags the port ignores.
            "-l" | "-host" | "-dct" | "-im" | "-ll" => {
                let _ = take_value(&mut i, a)?;
            }
            // Boolean switches the port ignores.
            "-da" | "-dl" => {}
            _ if is_generic_override(a) => {
                push_pair(&mut extra, "--set", a.trim_start_matches("--").to_string());
            }
            _ => extra.push(a.to_string()),
        }
        i += 1;
    }

    let dsn = dsn.ok_or(LegacyError::MissingDsn)?;
    let mut out: Vec<String> = Vec::new();
    if let Some(report) = drc_output {
        out.push("drc".into());
        out.push(dsn);
        if let Some(s) = ses {
            push_pair(&mut out, "--ses", s);
        }
        if let Some(r) = rules {
            push_pair(&mut out, "--rules", r);
        }
        push_pair(&mut out, "-o", report);
    } else {
        let output = output.ok_or(LegacyError::MissingOutput)?;
        out.push("route".into());
        out.push(dsn);
        if let Some(s) = ses {
            push_pair(&mut out, "--ses", s);
        }
        if let Some(r) = rules {
            push_pair(&mut out, "--rules", r);
        }
        push_pair(&mut out, "-o", output);
    }
    out.extend(extra);
    Ok(out)
}

/// `--section.field=value` (Java generic settings override).
fn is_generic_override(a: &str) -> bool {
    a.starts_with("--") && a.contains('.') && a.contains('=')
}

fn push_pair(v: &mut Vec<String>, flag: &str, value: String) {
    v.push(flag.to_string());
    v.push(value);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn passthrough_when_no_legacy_flags() {
        let argv = s(&["route", "a.dsn", "-o", "b.ses"]);
        assert_eq!(rewrite(&argv).unwrap(), argv);
    }

    #[test]
    fn de_do_become_route() {
        let argv = s(&["-de", "a.dsn", "-do", "b.ses"]);
        assert_eq!(
            rewrite(&argv).unwrap(),
            s(&["route", "a.dsn", "-o", "b.ses"])
        );
    }

    #[test]
    fn de_splits_plus_delimited_inputs() {
        let argv = s(&["-de", "a.dsn+a.ses+a.rules", "-do", "b.ses"]);
        assert_eq!(
            rewrite(&argv).unwrap(),
            s(&[
                "route", "a.dsn", "--ses", "a.ses", "--rules", "a.rules", "-o", "b.ses"
            ])
        );
    }

    #[test]
    fn numeric_and_strategy_flags_map() {
        let argv = s(&[
            "-de", "a.dsn", "-do", "b.ses", "-mp", "5", "-mt", "2", "-oit", "0.5", "-inc",
            "GND,VCC", "-us", "hybrid", "-hr", "1:1", "-is", "random", "-dr", "x.rules",
        ]);
        assert_eq!(
            rewrite(&argv).unwrap(),
            s(&[
                "route",
                "a.dsn",
                "--rules",
                "x.rules",
                "-o",
                "b.ses",
                "--max-passes",
                "5",
                "--threads",
                "2",
                "--optimizer-improvement-threshold",
                "0.5",
                "--ignore-net-classes",
                "GND,VCC",
                "--update-strategy",
                "hybrid",
                "--hybrid-ratio",
                "1:1",
                "--item-selection",
                "random",
            ])
        );
    }

    #[test]
    fn generic_section_field_overrides_become_set() {
        let argv = s(&["-de", "a.dsn", "-do", "b.ses", "--router.max_passes=7"]);
        assert_eq!(
            rewrite(&argv).unwrap(),
            s(&[
                "route",
                "a.dsn",
                "-o",
                "b.ses",
                "--set",
                "router.max_passes=7"
            ])
        );
    }

    #[test]
    fn drc_flag_becomes_drc_subcommand() {
        let argv = s(&["-de", "a.dsn+a.ses", "-drc", "r.json"]);
        assert_eq!(
            rewrite(&argv).unwrap(),
            s(&["drc", "a.dsn", "--ses", "a.ses", "-o", "r.json"])
        );
    }

    #[test]
    fn ignored_flags_are_dropped() {
        // -l, -da, -dl, -ll, -host, -dct, -im take a value; they are GUI/telemetry/binary-snapshot
        // options the port does not have.
        let argv = s(&[
            "-de", "a.dsn", "-do", "b.ses", "-l", "en", "-da", "-dl", "-host", "x", "-im", "y",
        ]);
        assert_eq!(
            rewrite(&argv).unwrap(),
            s(&["route", "a.dsn", "-o", "b.ses"])
        );
    }

    #[test]
    fn di_is_unsupported() {
        let argv = s(&["-di", "some/dir"]);
        assert!(matches!(rewrite(&argv), Err(LegacyError::Unsupported(f)) if f == "-di"));
    }

    #[test]
    fn de_without_do_or_drc_is_error() {
        let argv = s(&["-de", "a.dsn"]);
        assert!(matches!(rewrite(&argv), Err(LegacyError::MissingOutput)));
    }
}
