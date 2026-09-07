use copper_core::{
    MAX_TIMEOUT_SECONDS, convert_from_timespan_to_duration_format, job_timeout_deadline_from,
    parse_timespan, parse_timespan_seconds,
};

struct Row {
    input: &'static str,
    conv: &'static str,
    parse: Option<i64>,
    capped: Option<i64>,
}

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

#[test]
fn timespan_conversion_handles_the_input_table() {
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

#[test]
fn timespan_parser_handles_the_input_table() {
    for row in ROWS {
        match row.parse {
            Some(expected) => assert_eq!(parse_timespan_seconds(row.input), Ok(Some(expected))),
            None if row.input.trim().is_empty() => {
                assert_eq!(parse_timespan_seconds(row.input), Ok(None));
            }
            None => assert_eq!(
                parse_timespan_seconds(row.input)
                    .expect_err("invalid input")
                    .input,
                row.input
            ),
        }
    }
}

#[test]
fn timeout_inputs_produce_the_expected_deadlines() {
    let base = std::time::Instant::now();
    for row in ROWS {
        let deadline = job_timeout_deadline_from(Some(row.input), base);
        match row.capped {
            None => match deadline {
                Ok(none) => assert!(
                    none.is_none() && row.input.trim().is_empty(),
                    "job_timeout_deadline({:?}) should have no deadline",
                    row.input
                ),
                Err(error) => assert_eq!(error.input, row.input),
            },
            Some(seconds) => {
                let deadline = deadline
                    .unwrap_or_else(|e| panic!("job_timeout_deadline({:?}): {e}", row.input))
                    .unwrap_or_else(|| panic!("job_timeout_deadline({:?}) is None", row.input));
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
            }
        }
    }
}

#[test]
fn a_null_timeout_string_is_no_deadline() {
    assert_eq!(
        job_timeout_deadline_from(None, std::time::Instant::now()),
        Ok(None)
    );
}

#[test]
fn the_timeout_cap_is_one_day() {
    assert_eq!(MAX_TIMEOUT_SECONDS, 86_400);
}

#[test]
fn the_duration_view_loses_exactly_the_negatives() {
    assert_eq!(
        parse_timespan("1:30:00"),
        Some(std::time::Duration::from_secs(5400))
    );
    assert_eq!(parse_timespan("0"), Some(std::time::Duration::ZERO));
    assert_eq!(parse_timespan_seconds("-1"), Ok(Some(-1)));
    assert_eq!(parse_timespan("-1"), None, "Duration is unsigned");
    let base = std::time::Instant::now();
    let deadline = job_timeout_deadline_from(Some("-1"), base)
        .expect("`-1` parses")
        .expect("negative timeouts produce expired deadlines");
    assert!(
        deadline.stop_at < base,
        "a negative timeout is already expired"
    );
}
