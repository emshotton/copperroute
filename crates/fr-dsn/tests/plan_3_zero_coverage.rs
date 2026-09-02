//! The register for Plan 3's four zero-coverage paths (`docs/plan-3-handoff.md`'s
//! "The four zero-coverage Plan 3 paths" row, `docs/java-quirks.md`'s coverage-debt row).
//!
//! Plan 3 shipped four ported paths that **nothing executed**: not the 530-pair `sweep-p3t15.sh`,
//! not the seven `tests/reference` fixtures, not the 105-file corpus test. Each was transcribed
//! from the Java and reviewed by eye. Plan 8 Task 13 closed all four with a directed fixture per
//! path plus JVM ground truth, and this file is the list assertion that says so in one place: for
//! each row, the fixture exists, its transcript exists, and the named test exists in the suite
//! that owns the path.
//!
//! It is deliberately a **source** check rather than a runtime one. The four tests live in three
//! different integration binaries, so no single test can call them; what this file can do — and
//! does — is fail the moment one of them is renamed or deleted without this register being
//! updated, which is exactly the rot a future reader needs protection from.
//!
//! Ground truth for all four is `scripts/differential/java/probes/P8T13Probe.java` against the
//! pinned HEAD jar under JDK 25; that probe's header carries the exact `javac`/`java` invocation
//! and each transcript's `[jar-cli]` rows carry the task brief's `java -jar <HEAD jar> -de
//! <fixture> -do <out.ses>` acceptance run, exit code included.

/// One closed row of the register.
struct Row {
    /// The path's slug: `tests/data/p8t13-<path>.dsn` and `…/p8t13-directed-<path>.txt`.
    path: &'static str,
    /// The Java site the path is a port of.
    java: &'static str,
    /// The test that now executes it, and the suite it lives in.
    test: &'static str,
    suite: &'static str,
    /// The source of that suite, so the check needs no filesystem walk.
    source: &'static str,
    /// A companion fixture that isolates the row's finding by changing one token, when the row has
    /// one. Only quirk #105 does: `p8t13-via-net-numbers-control.dsn` is the hang fixture with
    /// `(net NORDERED 1)` on the via, and its transcript's `[jar-cli] exit=0` is what makes the
    /// hang attributable to the padded zero rather than to anything else about the file.
    control: Option<&'static str>,
}

/// The four rows of `docs/plan-3-handoff.md`'s zero-coverage list, taken verbatim from it.
///
/// The `instanceof Path` -> `PolylinePath` arm is deliberately **not** here: the handoff excludes
/// it ("it is unreachable in the port and documented as such — see Correction 3"), because
/// `transform_to_board_rel` answers `None` for a `PolylinePath` and the `continue` one line
/// earlier drops such an outline before the width/closed branch runs.
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
        java: "quirk #105 — Wiring.readViaScope's net-number loop (Wiring.java:684-687)",
        test: "read_via_scope_pads_a_multi_subnet_vias_net_numbers_with_zeros",
        suite: "tests/dsn_reader.rs",
        source: include_str!("dsn_reader.rs"),
        control: Some("via-net-numbers-control"),
    },
];

/// Every one of the four has a fixture, a JVM transcript and a live test naming both.
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

/// The register is exactly four rows long, and the handoff says so.
///
/// `docs/plan-3-handoff.md` is the authority for the list; if a fifth uncovered path is ever
/// found, it belongs in that document **and** here, in the same change.
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
