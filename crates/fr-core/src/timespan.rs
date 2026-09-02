//! `util/TextManager`'s timespan grammar and `RoutingJobSchedulerActionThread`'s timeout ladder.
//!
//! # Two Java methods, one of which is the grammar
//!
//! `TextManager.parseTimespanString` (`util/TextManager.java:83-93`) is eleven lines of wrapper:
//! a null/blank guard, `Duration.parse(convertFromTimespanToDurationFormat(s))`, and a
//! `catch (DateTimeParseException) { return null; }`. **All of the grammar** lives in
//! `convertFromTimespanToDurationFormat` (`:101-118`), which splits on `':'` and glues the parts
//! into an ISO-8601 duration string — `HH:mm:ss` -> `PTaHbMcS`, `mm:ss` -> `PTaMbS`,
//! `ss` -> `PTaS` — and, for **any other part count, returns the bare string `"PT"`**, which
//! `Duration.parse` rejects. That last arm is the one a reader guesses wrong: it is not an error
//! path in `convertFromTimespanToDurationFormat`, it is a silent fall-through.
//!
//! # Scan ruling R11: the draft's Java premise was false
//!
//! The plan draft said the method "returns a `Duration` and **throws**", caught at the call site.
//! It does neither. It is `public static Long parseTimespanString(String)`, it answers the
//! duration **in whole seconds as a `Long`**, it answers **`null`** for a `null`/blank input
//! (`:84-86`) and for any `DateTimeParseException` (`:90-92`), and its one call site
//! (`management/jobs/RoutingJobSchedulerActionThread.java:44`) has no `try`/`catch` — it
//! null-checks at `:45`. `Duration` is an internal intermediate only.
//!
//! # …and the consequence the plan did not draw: `Long` is **signed**
//!
//! `Duration.parse("PT-1S")` succeeds, so `parseTimespanString("-1")` answers `-1`, and the call
//! site then computes `job.timeoutAt = job.startedAt.plusSeconds(-1)` — a deadline **in the
//! past**, which the monitor thread trips on its first tick. [`std::time::Duration`] is unsigned
//! and cannot carry that answer, so the exact transcription is
//! [`parse_timespan_seconds`] (`Option<i64>`) and the plan's declared
//! [`parse_timespan`] is the lossy `Duration` view of it, documented at its own site.
//! **Every port decision reads the seconds form**: [`job_timeout_deadline`] does.
//!
//! # The ladder
//!
//! `threadAction:43-52`, in order: parse; if non-`null`, clamp **only from above** at
//! `MAX_TIMEOUT` (`:24`, transcribed from the file as `24 * 60 * 60`); then
//! `startedAt.plusSeconds(timeout)`. `GRACE_PERIOD` (`:25`, transcribed as `30`) is **not**
//! applied here — it belongs to the monitor thread (`:77`), and in the port it lands on
//! [`crate::cancel::Deadline::timed_out_at`] and never on `stop_at`.

use std::time::Duration;

use crate::cancel::Deadline;

/// `RoutingJobSchedulerActionThread.MAX_TIMEOUT` (`management/jobs/RoutingJobSchedulerActionThread.java:24`)
/// — `private static final long MAX_TIMEOUT = 24 * 60 * 60; // 24 hours`, transcribed from the
/// file rather than from the plan's prose, per the task brief.
pub const MAX_TIMEOUT_SECONDS: i64 = 24 * 60 * 60;

/// `RoutingJobSchedulerActionThread.GRACE_PERIOD` (`:25`) — `private static final int
/// GRACE_PERIOD = 30; // 30 seconds`. The monitor thread calls `job.thread.requestStop()` at
/// `:75` and only writes `job.state = RoutingJobState.TIMED_OUT` at `:84`, after spinning while
/// `Instant.now().isBefore(job.timeoutAt.plusSeconds(GRACE_PERIOD))` (`:77`). The port has no
/// monitor thread, so the grace is arithmetic on [`Deadline`].
pub const GRACE_PERIOD_SECONDS: i64 = 30;

/// Port of `TextManager.convertFromTimespanToDurationFormat` (`util/TextManager.java:101-118`).
///
/// `timespanString.split(":")` then a `StringBuilder` seeded with `"PT"`:
///
/// | `parts.length` | appended |
/// |---|---|
/// | 3 | `p0` `H` `p1` `M` `p2` `S` |
/// | 2 | `p0` `M` `p1` `S` |
/// | 1 | `p0` `S` |
/// | anything else | *nothing* — the answer is the bare `"PT"` |
///
/// # `String.split` is not `str::split`, and the difference decides three of the probe's rows
///
/// Java's `String.split(String)` with a zero `limit` **discards trailing empty strings**
/// (`java.lang.String.split(String,int)`), *except* that a string in which the separator never
/// matches is returned whole, empty or not. Measured against the HEAD jar
/// (`crates/fr-core/tests/data/p8t0-timespans.txt`):
///
/// | input | Java `split(":")` | this function |
/// |---|---|---|
/// | `"1:"` | `["1"]` — length **1**, not 2 | `"PT1S"` |
/// | `":"` | `[]` — the **empty** array | `"PT"` |
/// | `":1"` | `["", "1"]` — a leading empty survives | `"PTM1S"` |
/// | `""` | `[""]` — length **1**, because the separator never matched | `"PTS"` |
///
/// That last row is the one a trailing-empty trim gets wrong: `"".split(":")` is not `[]`.
/// (`parseTimespanString` never reaches it — `:84-86`'s `isBlank` guard fires first — but
/// `convertFromTimespanToDurationFormat` is `public static` and this is its answer.)
///
/// The single-argument `split` also treats its argument as a **regex**; `":"` has no
/// metacharacter, so a plain separator split agrees with it.
pub fn convert_from_timespan_to_duration_format(timespan_string: &str) -> String {
    // `String.split(":")` — see the doc comment's table.
    let mut parts: Vec<&str> = timespan_string.split(':').collect();
    if timespan_string.contains(':') {
        while parts.last() == Some(&"") {
            parts.pop();
        }
    }

    let mut duration_string = String::from("PT");
    match parts.len() {
        // `:103-110`.
        3 => {
            duration_string.push_str(parts[0]);
            duration_string.push('H');
            duration_string.push_str(parts[1]);
            duration_string.push('M');
            duration_string.push_str(parts[2]);
            duration_string.push('S');
        }
        // `:111-112`.
        2 => {
            duration_string.push_str(parts[0]);
            duration_string.push('M');
            duration_string.push_str(parts[1]);
            duration_string.push('S');
        }
        // `:113-114`.
        1 => {
            duration_string.push_str(parts[0]);
            duration_string.push('S');
        }
        // `:101-117` has no `else`: 0 parts and 4-or-more parts both leave the builder at `"PT"`,
        // which `Duration.parse` rejects with a `DateTimeParseException`.
        _ => {}
    }
    duration_string
}

/// Port of `TextManager.parseTimespanString` (`util/TextManager.java:83-93`) — **Plan 7's**.
///
/// # This is a re-export, and the reason matters
///
/// The task brief asks Task 0 to port this method. It is **already ported**: Plan 7 Task 12
/// landed it as `fr_router::pipeline::parse_timespan_seconds`
/// (`crates/fr-router/src/pipeline/fanout.rs`), `pub`, because `BatchFanout.fanoutBoard:94-99`
/// and `BatchOptimizer.runBatchLoop:153-159` are two more readers of the same method inside
/// Plan 7's own scope — and its quirk row **#224** (`FanoutSettings.timeout`'s own javadoc gives
/// `"5m"` and `"300s"`, neither of which the method parses) is pinned by
/// `crates/fr-router/tests/fanout.rs` against a JDK 25 `Duration.parse`.
///
/// Plan-8 ruling 1 says `fr-core` **re-exports rather than moves**, and the plan's §Interfaces
/// rule says a task that finds itself re-declaring an earlier task's produced item has made an
/// error and must report it rather than shadow it. So this is `pub use`, and the finding is in
/// the Task 0 report.
///
/// # Its signature is `Option<i64>`, and the brief's `Option<Duration>` cannot be
///
/// Java's return type is a signed `Long` and the negative values are reachable
/// (`parseTimespanString("-1")` really is `-1`, measured — `tests/data/p8t0-timespans.txt`), so
/// `Duration` cannot carry the answer. Plan 7 reached the same shape independently. See
/// [`parse_timespan`] for the lossy `Duration` view the plan's §Interfaces block declared, and
/// why nothing on a decision path reads it.
pub use fr_router::pipeline::parse_timespan_seconds;

/// The plan's declared Task-0 interface: [`parse_timespan_seconds`] seen as a
/// [`std::time::Duration`].
///
/// **Lossy, deliberately, and never on a decision path.** [`Duration`] is unsigned, so a Java
/// answer below zero — which `parseTimespanString("-1")` really does produce, see the module doc
/// — cannot be represented and comes back as `None`, folding "an expired timeout" into
/// "no timeout". [`job_timeout_deadline`] therefore reads [`parse_timespan_seconds`], not this,
/// and so must any later task that turns a timespan into behaviour rather than into a display
/// string.
pub fn parse_timespan(timespan_string: &str) -> Option<Duration> {
    match parse_timespan_seconds(timespan_string) {
        Some(seconds) if seconds >= 0 => Some(Duration::from_secs(seconds as u64)),
        // Java answers a negative here; `Duration` cannot. See the doc comment.
        Some(_) | None => None,
    }
}

/// Port of `RoutingJobSchedulerActionThread.threadAction`'s timeout ladder
/// (`management/jobs/RoutingJobSchedulerActionThread.java:43-52`), measured from `base`.
///
/// ```java
/// Long timeout = TextManager.parseTimespanString(job.routerSettings.jobTimeoutString);  // :44
/// if (timeout != null) {                                                                // :45
///   if (timeout > MAX_TIMEOUT) { timeout = MAX_TIMEOUT; }                                // :47-49
///   job.timeoutAt = job.startedAt.plusSeconds(timeout);                                  // :51
/// }
/// ```
///
/// Note what is **not** there: no lower clamp (a negative timeout survives, and is already
/// expired), and no `GRACE_PERIOD` — `:47-49` caps from above and nothing else. The grace is the
/// monitor thread's (`:77`) and lands on [`Deadline::timed_out_at`].
///
/// `None` — Java's `job.timeoutAt == null` — is "no job timeout", which is the CLI's default and
/// what every parity run uses.
pub fn job_timeout_deadline_from(
    timeout_string: Option<&str>,
    base: std::time::Instant,
) -> Option<Deadline> {
    // `:44`. Java passes `job.routerSettings.jobTimeoutString`, which may be `null`; the `None`
    // arm here is that null, and `parse_timespan_seconds` handles the blank one.
    let mut timeout = parse_timespan_seconds(timeout_string?)?;
    // `:47-49`.
    if timeout > MAX_TIMEOUT_SECONDS {
        timeout = MAX_TIMEOUT_SECONDS;
    }
    // `:51`.
    Some(Deadline::from_base(base, timeout))
}

/// [`job_timeout_deadline_from`] with `base` read from the clock, which is what
/// `job.startedAt = Instant.now()` (`:38`) is.
pub fn job_timeout_deadline(timeout_string: Option<&str>) -> Option<Deadline> {
    job_timeout_deadline_from(timeout_string, std::time::Instant::now())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_literals_are_the_files_own() {
        // RoutingJobSchedulerActionThread.java:24, :25 — transcribed, not inferred.
        assert_eq!(MAX_TIMEOUT_SECONDS, 86_400);
        assert_eq!(GRACE_PERIOD_SECONDS, 30);
    }

    #[test]
    fn the_grammar_is_convert_from_timespan_to_duration_format() {
        // TextManager.java:103-114 — the three arms.
        assert_eq!(
            convert_from_timespan_to_duration_format("1:30:00"),
            "PT1H30M00S"
        );
        assert_eq!(
            convert_from_timespan_to_duration_format("90:00"),
            "PT90M00S"
        );
        assert_eq!(convert_from_timespan_to_duration_format("90"), "PT90S");
        // …and the silent fall-through at 0 and >= 4 parts.
        assert_eq!(convert_from_timespan_to_duration_format("1:2:3:4"), "PT");
        assert_eq!(convert_from_timespan_to_duration_format(":"), "PT");
    }

    #[test]
    fn java_split_drops_the_trailing_empty_run() {
        // "1:" is length 1 in Java, not 2 — so it is the `ss` arm, not the `mm:ss` one.
        assert_eq!(convert_from_timespan_to_duration_format("1:"), "PT1S");
        // A leading empty survives, so ":1" is the two-part arm with an empty minutes field.
        assert_eq!(convert_from_timespan_to_duration_format(":1"), "PTM1S");
    }

    #[test]
    fn the_cap_is_applied_from_above_only() {
        let base = std::time::Instant::now();
        // 25 h -> capped to 24 h.
        let capped = job_timeout_deadline_from(Some("25:00:00"), base).expect("parses");
        assert_eq!(capped.stop_at, base + Duration::from_secs(86_400));
        // A negative is NOT clamped: `:47-49` has no lower arm.
        assert_eq!(parse_timespan_seconds("-1"), Some(-1));
    }
}
