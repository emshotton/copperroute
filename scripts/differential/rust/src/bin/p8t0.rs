//! Plan 8 Task 0's Rust half: `copper_core::timespan` against
//! `app.freerouting.util.P8T0Probe`'s reading of the HEAD jar.
//!
//! Prints exactly the lines `P8T0Probe.main` prints, so `scripts/differential/run.sh p8t0`
//! diffs them line for line. The thirty inputs are duplicated here rather than read from a
//! shared file, for the reason `batch_parity.rs`'s `STEMS` is duplicated: a driver that reads
//! its own expectations from the file it also writes can agree with itself while disagreeing
//! with the jar.
//!
//! The `CAPPED` and `OFFSET` columns are the Java side's
//! `RoutingJobSchedulerActionThread.java:45-51` arithmetic with `startedAt` pinned to
//! `Instant.EPOCH`. `copper_core::job_timeout_deadline_from` performs that on a
//! [`std::time::Instant`], which has no epoch to print, so this driver reproduces the same two
//! lines of arithmetic on the `i64` directly — the seam being
//! `copper_core::MAX_TIMEOUT_SECONDS`, which is the constant under test.

fn main() {
    // The two literals, which the Java half reads out of the jar by reflection.
    println!("MAX_TIMEOUT\t{}", copper_core::MAX_TIMEOUT_SECONDS);
    println!("ROWS\t{}", INPUTS.len());

    for input in INPUTS {
        let conv = copper_core::convert_from_timespan_to_duration_format(input);
        let value = copper_core::parse_timespan_seconds_java(input);
        let parsed = value.map_or_else(|| "null".to_string(), |v| v.to_string());

        // RoutingJobSchedulerActionThread.java:45-51.
        let (capped, offset) = match value {
            None => ("null".to_string(), "null".to_string()),
            Some(mut timeout) => {
                if timeout > copper_core::MAX_TIMEOUT_SECONDS {
                    timeout = copper_core::MAX_TIMEOUT_SECONDS;
                }
                // `Instant.EPOCH.plusSeconds(t).getEpochSecond()` is `t`, and Java's overflow arm
                // needs |t| near `Long.MAX_VALUE`, which the field parse rejects first.
                (timeout.to_string(), timeout.to_string())
            }
        };

        println!(
            "RAW\t{}\tCONV\t{}\tPARSE\t{}\tCAPPED\t{}\tOFFSET\t{}",
            quote(input),
            conv,
            parsed,
            capped,
            offset
        );
    }
}

/// `P8T0Probe.quote` — the same rendering, so a trailing space diffs visibly.
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

/// `P8T0Probe.INPUTS`, in the same order.
const INPUTS: [&str; 30] = [
    "1:30:00",
    "90",
    "1.5",
    "1:2:3:4",
    "x",
    "",
    "25:00:00",
    "-1",
    " 1:00 ",
    "   ",
    ":",
    "1:",
    ":1",
    "0",
    "00:00:00",
    "24:00:00",
    "24:00:01",
    "1:60",
    "0:0:90",
    "1:5.5",
    "1.5:00",
    "-1.5",
    "-0.5",
    "+90",
    "1:-30",
    "1:2:3",
    "99999999999999999999",
    "PT1H",
    "1:00:00.5",
    "01:00:00",
];
