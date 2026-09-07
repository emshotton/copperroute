use copper_dsn::format::{DSN_RESERVED, IdentifierType, IndentFileWriter, SES_RESERVED};

fn dsn_id() -> IdentifierType {
    IdentifierType::new(
        DSN_RESERVED.iter().map(|s| s.to_string()).collect(),
        "\"".to_string(),
    )
}

fn ses_id() -> IdentifierType {
    IdentifierType::new(
        SES_RESERVED.iter().map(|s| s.to_string()).collect(),
        "\"".to_string(),
    )
}

fn write_with(id: &IdentifierType, name: &str) -> String {
    let mut w = IndentFileWriter::new(Vec::new());
    id.write(name, &mut w);
    w.flush().expect("flush should not fail for a Vec<u8> sink");
    String::from_utf8(w.into_inner()).expect("output must be valid UTF-8")
}

const CASES: &[(&str, &str, &str)] = &[
    ("\"abc\"", "ab", "ab"),
    ("\"ab\"", "a", "a"),
    ("", "", ""),
    ("F.Cu", "F.Cu", "F.Cu"),
    ("D-", "\"D-\"", "\"D-\""),
    (
        "Via[0-1]_600:300_um",
        "\"Via[0-1]_600:300_um\"",
        "\"Via[0-1]_600:300_um\"",
    ),
    ("keepout_1", "\"keepout_1\"", "\"keepout_1\""),
    ("KiCad's Pcbnew", "\"KiCad's Pcbnew\"", "\"KiCad's Pcbnew\""),
    ("2N3904", "\"2N3904\"", "\"2N3904\""),
    ("-5V", "\"-5V\"", "\"-5V\""),
    ("Паяльная", "\"Паяльная\"", "\"Паяльная\""),
    ("a\"b", "ab", "ab"),
    ("net/1", "net/1", "\"net/1\""),
    ("a~b", "a~b", "\"a~b\""),
    ("N$1", "N$1", "N$1"),
    ("GND", "GND", "GND"),
];

#[test]
fn dsn_reserved_set_matches_jvm() {
    let id = dsn_id();
    for (input, expected_dsn, _) in CASES {
        assert_eq!(
            write_with(&id, input),
            *expected_dsn,
            "DSN output for input {input:?}"
        );
    }
}

#[test]
fn ses_reserved_set_matches_jvm() {
    let id = ses_id();
    for (input, _, expected_ses) in CASES {
        assert_eq!(
            write_with(&id, input),
            *expected_ses,
            "SES output for input {input:?}"
        );
    }
}
