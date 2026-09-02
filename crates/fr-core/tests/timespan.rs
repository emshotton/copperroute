//! Plan 8 Task 0's timespan table: `TextManager.parseTimespanString` and
//! `RoutingJobSchedulerActionThread`'s cap, thirty rows, pinned to the HEAD jar.
//!
//! # Where the expectations come from
//!
//! `scripts/differential/java/probes/P8T0Probe.java` runs both Java methods on the thirty inputs
//! below against `../freerouting/build/libs/freerouting-current-executable.jar` (JDK 25,
//! `-Djava.awt.headless=true -Duser.language=en -Duser.country=US
//! -XX:+UnlockExperimentalVMOptions -XX:hashCode=2`) and prints one row each. Its output is
//! committed verbatim as `tests/data/p8t0-timespans.txt`, and
//! `scripts/differential/run.sh p8t0` diffs it live against `scripts/differential/rust/src/bin/p8t0.rs`
//! — **MATCH on all 30 rows** at the jar whose `Build-Revision` the transcript's header records.
//!
//! # Why the table is literals AND the transcript is re-checked
//!
//! [`ROWS`] is the transcript transcribed into Rust, one case per row, as the brief asks: a test
//! that reads its expectations out of a file can agree with itself while disagreeing with the
//! jar. [`the_committed_transcript_still_says_what_this_table_says`] then re-reads the file and
//! requires the two to agree, so a regenerated transcript cannot drift away from the table
//! silently either.
//!
//! # What each row proves
//!
//! Beyond the ten inputs the brief names, the table's own surprises — every one of them measured,
//! not reasoned:
//!
//! * `""` -> `"PTS"`, not `"PT"`. `"".split(":")` is `[""]`, length **1**: Java's split only drops
//!   trailing empties when the separator matched at all. `":"` really is the empty array.
//! * `"1:"` is the **one**-part arm (`"PT1S"` = 1 s), not `mm:ss`.
//! * `"1:60"` is 120 s and `"0:0:90"` is 90 s — out-of-range components are not normalised, they
//!   are just added up.
//! * `"1:5.5"` parses (only the seconds field may be fractional) and `"1.5:00"` does not.
//! * `"-1.5"` is **-2**, not -1: `Duration.getSeconds()` floors.
//! * `"-1"` is -1 and the cap does **not** clamp it — `:47-49` has no lower arm, so a negative
//!   `--job-timeout` is a deadline in the past.
//! * `"24:00:01"` caps to 86 400; `"24:00:00"` is already exactly `MAX_TIMEOUT`.
//! * `" 1:00 "` fails: `Duration.parse` has no tolerance for whitespace inside the literal.

use fr_core::{
    GRACE_PERIOD_SECONDS, MAX_TIMEOUT_SECONDS, convert_from_timespan_to_duration_format,
    job_timeout_deadline_from, parse_timespan, parse_timespan_seconds,
};

/// One row of `tests/data/p8t0-timespans.txt`.
struct Row {
    /// The `RAW` column, as the string itself rather than as its C-quoted rendering.
    input: &'static str,
    /// The `CONV` column — `convertFromTimespanToDurationFormat`'s answer.
    conv: &'static str,
    /// The `PARSE` column — `parseTimespanString`'s `Long`, or `None` for Java's `null`.
    /// **Never an exception**: scan ruling R11, and thirty rows of evidence.
    parse: Option<i64>,
    /// The `CAPPED` column — `:47-49`'s clamp applied to `parse`.
    capped: Option<i64>,
}

/// The thirty rows of `tests/data/p8t0-timespans.txt`, in its order.
const ROWS: &[Row] = &[
    Row {
        input: "1:30:00",
        conv: "PT1H30M00S",
        parse: Some(5400),
        capped: Some(5400),
    },
    Row {
        input: "90",
        conv: "PT90S",
        parse: Some(90),
        capped: Some(90),
    },
    Row {
        input: "1.5",
        conv: "PT1.5S",
        parse: Some(1),
        capped: Some(1),
    },
    Row {
        input: "1:2:3:4",
        conv: "PT",
        parse: None,
        capped: None,
    },
    Row {
        input: "x",
        conv: "PTxS",
        parse: None,
        capped: None,
    },
    Row {
        input: "",
        conv: "PTS",
        parse: None,
        capped: None,
    },
    Row {
        input: "25:00:00",
        conv: "PT25H00M00S",
        parse: Some(90000),
        capped: Some(86400),
    },
    Row {
        input: "-1",
        conv: "PT-1S",
        parse: Some(-1),
        capped: Some(-1),
    },
    Row {
        input: " 1:00 ",
        conv: "PT 1M00 S",
        parse: None,
        capped: None,
    },
    Row {
        input: "   ",
        conv: "PT   S",
        parse: None,
        capped: None,
    },
    Row {
        input: ":",
        conv: "PT",
        parse: None,
        capped: None,
    },
    Row {
        input: "1:",
        conv: "PT1S",
        parse: Some(1),
        capped: Some(1),
    },
    Row {
        input: ":1",
        conv: "PTM1S",
        parse: None,
        capped: None,
    },
    Row {
        input: "0",
        conv: "PT0S",
        parse: Some(0),
        capped: Some(0),
    },
    Row {
        input: "00:00:00",
        conv: "PT00H00M00S",
        parse: Some(0),
        capped: Some(0),
    },
    Row {
        input: "24:00:00",
        conv: "PT24H00M00S",
        parse: Some(86400),
        capped: Some(86400),
    },
    Row {
        input: "24:00:01",
        conv: "PT24H00M01S",
        parse: Some(86401),
        capped: Some(86400),
    },
    Row {
        input: "1:60",
        conv: "PT1M60S",
        parse: Some(120),
        capped: Some(120),
    },
    Row {
        input: "0:0:90",
        conv: "PT0H0M90S",
        parse: Some(90),
        capped: Some(90),
    },
    Row {
        input: "1:5.5",
        conv: "PT1M5.5S",
        parse: Some(65),
        capped: Some(65),
    },
    Row {
        input: "1.5:00",
        conv: "PT1.5M00S",
        parse: None,
        capped: None,
    },
    Row {
        input: "-1.5",
        conv: "PT-1.5S",
        parse: Some(-2),
        capped: Some(-2),
    },
    Row {
        input: "-0.5",
        conv: "PT-0.5S",
        parse: Some(-1),
        capped: Some(-1),
    },
    Row {
        input: "+90",
        conv: "PT+90S",
        parse: Some(90),
        capped: Some(90),
    },
    Row {
        input: "1:-30",
        conv: "PT1M-30S",
        parse: Some(30),
        capped: Some(30),
    },
    Row {
        input: "1:2:3",
        conv: "PT1H2M3S",
        parse: Some(3723),
        capped: Some(3723),
    },
    Row {
        input: "99999999999999999999",
        conv: "PT99999999999999999999S",
        parse: None,
        capped: None,
    },
    Row {
        input: "PT1H",
        conv: "PTPT1HS",
        parse: None,
        capped: None,
    },
    Row {
        input: "1:00:00.5",
        conv: "PT1H00M00.5S",
        parse: Some(3600),
        capped: Some(3600),
    },
    Row {
        input: "01:00:00",
        conv: "PT01H00M00S",
        parse: Some(3600),
        capped: Some(3600),
    },
];

/// `convertFromTimespanToDurationFormat` (`util/TextManager.java:101-118`), row by row.
#[test]
fn the_grammar_matches_the_jar_on_all_thirty_rows() {
    for row in ROWS {
        assert_eq!(
            convert_from_timespan_to_duration_format(row.input),
            row.conv,
            "convertFromTimespanToDurationFormat({:?})",
            row.input
        );
    }
    assert_eq!(ROWS.len(), 30);
}

/// `parseTimespanString` (`:83-93`), row by row — the exact `Long`/`null` answer.
#[test]
fn parse_timespan_string_matches_the_jar_on_all_thirty_rows() {
    for row in ROWS {
        assert_eq!(
            parse_timespan_seconds(row.input),
            row.parse,
            "parseTimespanString({:?})",
            row.input
        );
    }
}

/// `threadAction:43-52`'s ladder, row by row: parse, cap from above, offset from `startedAt`.
#[test]
fn the_timeout_ladder_matches_the_jar_on_all_thirty_rows() {
    let base = std::time::Instant::now();
    for row in ROWS {
        let deadline = job_timeout_deadline_from(Some(row.input), base);
        match row.capped {
            None => assert!(
                deadline.is_none(),
                "job_timeout_deadline({:?}) should be Java's null timeoutAt",
                row.input
            ),
            Some(seconds) => {
                let deadline = deadline
                    .unwrap_or_else(|| panic!("job_timeout_deadline({:?}) is None", row.input));
                // `:51` — `job.startedAt.plusSeconds(timeout)`.
                let expected_stop = if seconds >= 0 {
                    base + std::time::Duration::from_secs(seconds as u64)
                } else {
                    base - std::time::Duration::from_secs(seconds.unsigned_abs())
                };
                assert_eq!(
                    deadline.stop_at, expected_stop,
                    "stop_at for {:?}",
                    row.input
                );
                // `:25`/`:77` — the grace, which `:43-52` does NOT apply.
                assert_eq!(
                    deadline.timed_out_at,
                    expected_stop + std::time::Duration::from_secs(GRACE_PERIOD_SECONDS as u64),
                    "timed_out_at for {:?}",
                    row.input
                );
            }
        }
    }
}

/// `None` — Java's `job.routerSettings.jobTimeoutString == null` — is "no job timeout", which is
/// what every parity run uses and what `RouterStop::new()` is.
#[test]
fn a_null_timeout_string_is_no_deadline() {
    assert!(job_timeout_deadline_from(None, std::time::Instant::now()).is_none());
}

/// The two literals are the file's, not the plan's (`RoutingJobSchedulerActionThread.java:24`,
/// `:25`), and the probe re-reads them out of the jar by reflection.
#[test]
fn the_two_literals_agree_with_the_jar() {
    let transcript = transcript();
    assert!(
        transcript.contains(&format!("MAX_TIMEOUT\t{MAX_TIMEOUT_SECONDS}\n")),
        "MAX_TIMEOUT disagrees with the jar's"
    );
    assert!(
        transcript.contains(&format!("GRACE_PERIOD\t{GRACE_PERIOD_SECONDS}\n")),
        "GRACE_PERIOD disagrees with the jar's"
    );
}

/// [`ROWS`] is the transcript, transcribed. This re-reads the file and requires the two to still
/// say the same thing, so a regenerated `p8t0-timespans.txt` cannot drift away from the table
/// without a failing test.
#[test]
fn the_committed_transcript_still_says_what_this_table_says() {
    let transcript = transcript();
    let rows: Vec<&str> = transcript
        .lines()
        .filter(|line| line.starts_with("RAW\t"))
        .collect();
    assert_eq!(rows.len(), ROWS.len(), "row count");

    for (line, row) in rows.iter().zip(ROWS) {
        let fields: Vec<&str> = line.split('\t').collect();
        // RAW <q> CONV <c> PARSE <p> CAPPED <k> OFFSET <o>
        assert_eq!(fields.len(), 10, "malformed transcript row: {line}");
        assert_eq!(fields[1], quote(row.input), "RAW column of {line}");
        assert_eq!(fields[3], row.conv, "CONV column of {line}");
        assert_eq!(
            fields[5],
            row.parse
                .map_or_else(|| "null".to_string(), |v| v.to_string()),
            "PARSE column of {line}"
        );
        assert_eq!(
            fields[7],
            row.capped
                .map_or_else(|| "null".to_string(), |v| v.to_string()),
            "CAPPED column of {line}"
        );
    }
}

/// The lossy `Duration` view of [`parse_timespan_seconds`], and the one input that proves it is
/// lossy: Java answers -1 for `"-1"`, [`std::time::Duration`] cannot hold it, and
/// [`parse_timespan`] therefore folds it into `None`. Nothing on a decision path reads this.
#[test]
fn the_duration_view_loses_exactly_the_negatives() {
    assert_eq!(
        parse_timespan("1:30:00"),
        Some(std::time::Duration::from_secs(5400))
    );
    assert_eq!(parse_timespan("0"), Some(std::time::Duration::ZERO));
    assert_eq!(parse_timespan_seconds("-1"), Some(-1));
    assert_eq!(parse_timespan("-1"), None, "Duration is unsigned");
    // …and the ladder, which is the decision path, keeps the sign.
    let base = std::time::Instant::now();
    let deadline = job_timeout_deadline_from(Some("-1"), base).expect("Java answers -1, not null");
    assert!(
        deadline.stop_at < base,
        "a negative timeout is already expired"
    );
}

fn transcript() -> String {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/p8t0-timespans.txt");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// `P8T0Probe.quote`, so the `RAW` column can be compared without unescaping it.
fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        if c == '"' || c == '\\' {
            out.push('\\');
            out.push(c);
        } else if (c as u32) < 0x20 || (c as u32) > 0x7e {
            out.push_str(&format!("\\u{:04x}", c as u32));
        } else {
            out.push(c);
        }
    }
    out.push('"');
    out
}
