struct Row {
    path: &'static str,
    java: &'static str,
    test: &'static str,
    suite: &'static str,
    source: &'static str,
    control: Option<&'static str>,
}

const ROWS: [Row; 4] = [
    Row {
        path: "was-is",
        java: "SesWriter.writeWasIs's swap body (SesWriter.java:188-215)",
        test: "ses_writer_writes_a_pins_line_for_every_swapped_pin",
        suite: "tests/parity_ses.rs",
        source: include_str!("parity_ses.rs"),
        control: None,
    },
    Row {
        path: "conduction-area",
        java: "SesWriter.writeConductionArea, quirk #110 (SesWriter.java:536-553)",
        test: "ses_writer_mixes_integer_boundary_and_double_hole_coordinates",
        suite: "tests/parity_ses.rs",
        source: include_str!("parity_ses.rs"),
        control: None,
    },
    Row {
        path: "lock-type",
        java: "Component.readLockType's `(lock_type position)` arm (Component.java:352-364)",
        test: "the_lock_type_position_arm_survives_a_whole_file_read",
        suite: "tests/placement_scope.rs",
        source: include_str!("placement_scope.rs"),
        control: None,
    },
    Row {
        path: "via-net-numbers",
        java: "quirk #105 — Wiring.readViaScope's net-number loop (Wiring.java:684-687), fixed in \
               Plan 9 Task 5",
        test: "a_multi_subnet_via_carries_every_net_number",
        suite: "tests/wiring.rs",
        source: include_str!("wiring.rs"),
        control: Some("via-net-numbers-control"),
    },
];

#[test]
fn every_plan_3_zero_coverage_path_has_a_directed_test() {
    let data = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data");
    for row in &ROWS {
        let fixture = format!("{data}/p8t13-{}.dsn", row.path);
        assert!(
            std::path::Path::new(&fixture).is_file(),
            "{}: the directed fixture {fixture} is missing",
            row.java
        );

        let transcript_path = format!("{data}/p8t13-directed-{}.txt", row.path);
        let transcript = std::fs::read_to_string(&transcript_path)
            .unwrap_or_else(|e| panic!("{}: {transcript_path}: {e}", row.java));
        assert!(
            transcript.contains("[jar-cli] cmd=java "),
            "{}: {transcript_path} carries no `[jar-cli]` acceptance run — a fixture only the \
             port accepts proves nothing about the port",
            row.java
        );
        assert!(
            transcript.contains("[read] Success"),
            "{}: the HEAD jar must read {}.dsn successfully",
            row.java,
            row.path
        );

        assert!(
            row.source.contains(&format!("fn {}()", row.test)),
            "{}: {} no longer defines `{}` — the path is uncovered again, or the register is \
             stale",
            row.java,
            row.suite,
            row.test
        );
        assert!(
            row.source.contains(&format!("p8t13-{}.dsn", row.path)),
            "{}: {} must name its fixture `p8t13-{}.dsn` in the test's doc comment",
            row.java,
            row.suite,
            row.path
        );

        let Some(control) = row.control else {
            continue;
        };
        let control_fixture = format!("{data}/p8t13-{control}.dsn");
        assert!(
            std::path::Path::new(&control_fixture).is_file(),
            "{}: the control fixture {control_fixture} is missing — without it the row's finding \
             is prose, not evidence",
            row.java
        );
        let control_transcript =
            std::fs::read_to_string(format!("{data}/p8t13-directed-{control}.txt"))
                .unwrap_or_else(|e| panic!("{}: p8t13-directed-{control}.txt: {e}", row.java));
        assert!(
            control_transcript.contains("[jar-cli] exit=0"),
            "{}: the control's whole job is to show the jar terminating on the same file",
            row.java
        );
        assert!(
            row.source.contains(&format!("p8t13-{control}.dsn")),
            "{}: {} must name the control fixture `p8t13-{control}.dsn` in the test's doc comment",
            row.java,
            row.suite
        );
    }
}

#[test]
fn the_register_is_the_four_rows_the_handoff_names() {
    let handoff = include_str!("../../../docs/plan-3-handoff.md");
    assert!(
        handoff.contains("The four zero-coverage Plan 3 paths"),
        "docs/plan-3-handoff.md no longer carries the register row this file mirrors"
    );
    for row in &ROWS {
        assert!(
            handoff.contains(&format!("p8t13-{}.dsn", row.path)),
            "docs/plan-3-handoff.md does not name the directed fixture p8t13-{}.dsn",
            row.path
        );
    }
    assert_eq!(ROWS.len(), 4, "the handoff's list is four rows long");
}
