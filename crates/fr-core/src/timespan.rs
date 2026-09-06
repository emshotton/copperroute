use std::time::Duration;

use crate::cancel::Deadline;

pub const MAX_TIMEOUT_SECONDS: i64 = 24 * 60 * 60;

pub fn convert_from_timespan_to_duration_format(timespan_string: &str) -> String {
    let mut parts: Vec<&str> = timespan_string.split(':').collect();
    if timespan_string.contains(':') {
        while parts.last() == Some(&"") {
            parts.pop();
        }
    }

    let mut duration_string = String::from("PT");
    match parts.len() {
        3 => {
            duration_string.push_str(parts[0]);
            duration_string.push('H');
            duration_string.push_str(parts[1]);
            duration_string.push('M');
            duration_string.push_str(parts[2]);
            duration_string.push('S');
        }
        2 => {
            duration_string.push_str(parts[0]);
            duration_string.push('M');
            duration_string.push_str(parts[1]);
            duration_string.push('S');
        }
        1 => {
            duration_string.push_str(parts[0]);
            duration_string.push('S');
        }
        _ => {}
    }
    duration_string
}

pub use fr_router::pipeline::{TimespanError, parse_timespan_seconds, parse_timespan_seconds_java};

pub fn parse_timespan(timespan_string: &str) -> Option<Duration> {
    match parse_timespan_seconds(timespan_string) {
        Ok(Some(seconds)) if seconds >= 0 => Some(Duration::from_secs(seconds as u64)),
        Ok(_) | Err(_) => None,
    }
}

pub fn job_timeout_deadline_from(
    timeout_string: Option<&str>,
    base: std::time::Instant,
) -> Result<Option<Deadline>, TimespanError> {
    let Some(timeout_string) = timeout_string else {
        return Ok(None);
    };
    let Some(mut timeout) = parse_timespan_seconds(timeout_string)? else {
        return Ok(None);
    };
    if timeout > MAX_TIMEOUT_SECONDS {
        timeout = MAX_TIMEOUT_SECONDS;
    }
    Ok(Some(Deadline::from_base(base, timeout)))
}

pub fn job_timeout_deadline(
    timeout_string: Option<&str>,
) -> Result<Option<Deadline>, TimespanError> {
    job_timeout_deadline_from(timeout_string, std::time::Instant::now())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_timeout_cap_is_one_day() {
        assert_eq!(MAX_TIMEOUT_SECONDS, 86_400);
    }

    #[test]
    fn the_grammar_is_convert_from_timespan_to_duration_format() {
        assert_eq!(
            convert_from_timespan_to_duration_format("1:30:00"),
            "PT1H30M00S"
        );
        assert_eq!(
            convert_from_timespan_to_duration_format("90:00"),
            "PT90M00S"
        );
        assert_eq!(convert_from_timespan_to_duration_format("90"), "PT90S");
        assert_eq!(convert_from_timespan_to_duration_format("1:2:3:4"), "PT");
        assert_eq!(convert_from_timespan_to_duration_format(":"), "PT");
    }

    #[test]
    fn split_drops_the_trailing_empty_run() {
        assert_eq!(convert_from_timespan_to_duration_format("1:"), "PT1S");
        assert_eq!(convert_from_timespan_to_duration_format(":1"), "PTM1S");
    }

    #[test]
    fn the_cap_is_applied_from_above_only() {
        let base = std::time::Instant::now();
        let capped = job_timeout_deadline_from(Some("25:00:00"), base)
            .expect("parses")
            .expect("a deadline");
        assert_eq!(capped.stop_at, base + Duration::from_secs(86_400));
        assert_eq!(parse_timespan_seconds("-1"), Ok(Some(-1)));
    }
}
