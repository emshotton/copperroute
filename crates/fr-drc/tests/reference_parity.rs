//! Plan 5 Task 9: the port's `-drc` JSON against the HEAD jar's, on eight boards.
//!
//! # What the references are
//!
//! `tests/reference/<stem>/drc.json` is the **verbatim** output of the real `-drc` CLI on the
//! clone's HEAD build (plan-5 rulings 1 and 10) — no post-processing, no hand edits. The rows,
//! the exact command per stem and the jar's identity are `tests/reference/drc-fixtures.txt` and
//! each stem's `drc.meta.txt`; `scripts/gen-drc-reference.sh` regenerates them.
//!
//! Driving the CLI rather than `DesignRulesChecker` alone is what puts a real `qualityScore` in
//! the reference: it comes from `BoardStatistics.getNormalizedScore` (`Freerouting.java:349`),
//! which spec §4 puts in `fr-core` and plan-5 ruling 5 leaves to Plan 8. `fr-drc` takes it as an
//! injected `Option<f64>`, so these tests read the reference's value and feed it back in — the
//! same trick `tests/report_json.rs` uses for `date`. Everything else in the document is the
//! port's own work.
//!
//! # The normalisation, and why it is so small
//!
//! [`parity::normalize_drc_json`] drops `date` and sorts each `unconnectedItems` entry's `items`
//! by numeric uuid. That is all: `violations` — array order *and* each entry's `items` — is
//! compared exactly, because it is this plan's bit-parity surface (ruling 3). The generator's
//! `--verify-hash-modes` sweep is the evidence that this is enough: seven of the eight stems come
//! out **byte-identical** across `-XX:hashCode=0,1,2,3,4`, i.e. the reference is a property of the
//! board and not of the JVM run that produced it.
//!
//! # The eighth stem
//!
//! `drc-natural-tone-preamp` is the exception the sweep finds, and it is a known one (quirk #146,
//! Task 4/7/8): `generateReport` folds `getAllUnconnectedItems`' `track_dangling` entries into
//! `violations` (`DesignRulesChecker.java:271-276`), and that phase's dedup (`:160`) drops
//! whichever dangling trace a net entry's hash-ordered `firstItem` happens to be.
//!
//! Its violation count lands in the range **113-115**, and only `-XX:hashCode=2` pins it: mode 2
//! is a constant, the one `Object.hashCode` source that reproduces run to run without being
//! derived from an object address, and it gives **115** — the committed reference. Modes 0 and 5
//! are PRNG-seeded and 1 and 4 are address-derived; mode 3 is a per-thread xorshift, deterministic
//! only within a single-threaded run. Across the three sweeps taken of this jar, every one of
//! those four produced a different count in at least one sweep while mode 2 held at 115. No
//! per-mode count is quoted for them — the range and mode 2's value are the whole of what is
//! stable.
//!
//! The port's ascending-id representatives (ruling 3) make three of that fixture's four
//! `Trace`-represented net entries dangling, so it emits 112, and
//! [`natural_tone_preamp_is_the_reference_minus_three_dangling_tracks`] requires the port's
//! document to be the reference with exactly those three entries deleted **in place** — pinned by
//! their item uuids, never by count alone. That is Task 8's prose claim (it checked this once, by
//! hand, and did not commit the golden) turned into a repeatable byte-level test.

use fr_board::prelude::*;
use fr_drc::DesignRulesChecker;
use fr_drc::report::{DrcCoordinates, DrcJsonFlavor, DrcReportOptions};
use fr_dsn::{BoardReadResult, CoordinateTransform, DsnReadOptions};

// ---------------------------------------------------------------------------------------------
// The fixture table
// ---------------------------------------------------------------------------------------------

/// One row of `tests/reference/drc-fixtures.txt`: `stem|dsn|rules|ses`, the last two optional.
struct Row {
    stem: String,
    dsn: String,
    rules: Option<String>,
    ses: Option<String>,
}

/// Reads the generator's own fixture table, so the tests and the references cannot drift apart.
fn rows() -> Vec<Row> {
    let path = parity::workspace_root().join("tests/reference/drc-fixtures.txt");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let mut fields = line.split('|');
            let mut next = || fields.next().unwrap_or_default().trim().to_string();
            let (stem, dsn, rules, ses) = (next(), next(), next(), next());
            Row {
                stem,
                dsn,
                rules: (!rules.is_empty()).then_some(rules),
                ses: (!ses.is_empty()).then_some(ses),
            }
        })
        .collect()
}

fn row(stem: &str) -> Row {
    rows()
        .into_iter()
        .find(|row| row.stem == stem)
        .unwrap_or_else(|| panic!("{stem} is not in tests/reference/drc-fixtures.txt"))
}

/// `new File(globalSettings.initialInputFile).getName()` (`Freerouting.java:339`).
fn base_name(path: &str) -> &str {
    path.rsplit('/').next().expect("a non-empty path")
}

// ---------------------------------------------------------------------------------------------
// The port's side of the pipeline: `Freerouting.initializeDrc` minus the CLI
// ---------------------------------------------------------------------------------------------

/// `BoardLoader.loadBoardIfNeeded` → `HeadlessBoardManager.loadFromSpecctraDsn`
/// (`BoardLoader.java:19-52`, `:48`), plus the transform `Structure.createBoard` built, which Java reaches
/// through `board.communication` and the port takes as a parameter (plan-5 ruling 7).
fn read_dsn(rel_path: &str) -> (Board, CoordinateTransform) {
    let path = parity::java_dir().join(rel_path);
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("cannot read fixture {}: {e}", path.display()));
    let name = base_name(rel_path);
    match fr_dsn::read_board(&bytes[..], None, Some(name), &DsnReadOptions::default()) {
        BoardReadResult::Success {
            board,
            coordinate_transform,
            ..
        }
        | BoardReadResult::OutlineMissing {
            board,
            coordinate_transform,
            ..
        } => (
            *board.unwrap_or_else(|| panic!("{name} produced no board")),
            coordinate_transform.unwrap_or_else(|| panic!("{name} produced no transform")),
        ),
        other => panic!("{name} did not read: {other:?}"),
    }
}

/// The board `Freerouting.initializeDrc` hands to `DesignRulesChecker`: the DSN, then the
/// `.rules` file if one was given (`Freerouting.java:277-292`), then the session file
/// (`:297-329`) — in that order, because the rules can change clearances the SES's wires are then
/// checked against.
fn load_board(row: &Row) -> (Board, CoordinateTransform) {
    let (mut board, transform) = read_dsn(&row.dsn);

    if let Some(rules) = &row.rules {
        let path = parity::java_dir().join(rules);
        let file = std::fs::File::open(&path)
            .unwrap_or_else(|e| panic!("cannot open {}: {e}", path.display()));
        // `designName` is `drcJob.name` (`Freerouting.java:283`), which `RoutingJob.setInput`
        // fills from `input.getFilenameWithoutExtension()` (`RoutingJob.java:457`) — so it is the
        // base name **without** `.dsn`. That matters for fidelity even though the port ignores the
        // parameter: `RulesReader` compares it against the `(rules PCB <name>` header and warns on
        // a mismatch (`RulesReader.java:100-110`), and `Issue593-BBD_Mars-64.rules` spells that
        // header *with* the extension, so the reference run took the mismatch branch — as its
        // `java.log` records. Passing the extension-ful name here would take the other one.
        //
        // Java passes `drcJob.routerSettings` as the fourth argument (`Freerouting.java:284-285`).
        // The port's `target_settings` receives only the file's `(autoroute_settings …)`, which
        // reaches the router and never the board, so the DRC path can pass `None`.
        let design_name = base_name(&row.dsn)
            .strip_suffix(".dsn")
            .unwrap_or_else(|| base_name(&row.dsn));
        let read = fr_dsn::rules_reader::read(file, design_name, &mut board, &transform, None)
            .unwrap_or_else(|e| panic!("{} did not read: {e:?}", path.display()));
        assert!(read, "{} was rejected by the rules reader", path.display());
    }

    if let Some(ses) = &row.ses {
        let path = parity::java_dir().join(ses);
        let file = std::fs::File::open(&path)
            .unwrap_or_else(|e| panic!("cannot open {}: {e}", path.display()));
        let summary = fr_dsn::ses_reader::read(file, &mut board, &transform)
            .unwrap_or_else(|e| panic!("{} did not read: {e:?}", path.display()));
        assert_eq!(
            summary.errors_encountered,
            0,
            "{} imported with errors",
            path.display()
        );
    }

    (board, transform)
}

/// The port's `-drc` document for one row, with the three values `Freerouting.initializeDrc`
/// injects taken out of the reference (plan-5 ruling 5): the `date`
/// (`ZonedDateTime.now()`, `KiCadDrcReport.java:70`), the `Constants.FREEROUTING_VERSION` the jar
/// was built with, and the `qualityScore` `BoardStatistics` computes (`Freerouting.java:349`),
/// which is Plan 8's.
fn port_json(row: &Row, reference: &parity::DrcReportDoc) -> String {
    let source = base_name(&row.dsn).to_string();
    assert_eq!(
        source, reference.source,
        "the CLI's `source` is the input file's base name (Freerouting.java:339)"
    );
    let version = reference
        .freerouting_version
        .strip_prefix("Freerouting ")
        .unwrap_or_else(|| {
            panic!(
                "`generateReport` prefixes \"Freerouting \" (DesignRulesChecker.java:212-213); \
                 got {}",
                reference.freerouting_version
            )
        })
        .to_string();
    let options = DrcReportOptions {
        source,
        // `Freerouting.initializeDrc` hard-codes `"mm"` and offers no way to change it
        // (`Freerouting.java:335-336`, quirk #151).
        coordinate_unit: "mm".to_string(),
        date: reference
            .date
            .clone()
            .expect("the reference carries a date"),
        freerouting_version: version,
        quality_score: reference.quality_score,
    };

    let (mut board, transform) = load_board(row);
    let coords = DrcCoordinates {
        board_unit: board.communication.unit,
        transform,
    };
    DesignRulesChecker::new(&mut board)
        .report_to_json(&coords, &options, DrcJsonFlavor::FreeroutingHead)
        .expect("the report serialises")
}

// ---------------------------------------------------------------------------------------------
// The comparison
// ---------------------------------------------------------------------------------------------

fn reference_path(stem: &str) -> std::path::PathBuf {
    parity::reference(stem, "drc.json")
}

fn read_reference(stem: &str) -> parity::DrcReportDoc {
    let path = reference_path(stem);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read reference {}: {e}", path.display()));
    parity::parse_drc_json(&text)
        .unwrap_or_else(|e| panic!("{} is not a HEAD-flavor DRC report: {e}", path.display()))
}

/// Writes the normalised reference to `tests/reference/_scratch/` (gitignored) and compares the
/// port's normalised document against it, so a failure prints a diff and leaves both halves on
/// disk.
fn assert_normalised_parity(stem: &str, port: &str, expected: &mut parity::DrcReportDoc) {
    let expected_text = parity::normalize_drc_doc(expected).expect("the reference re-serialises");
    let scratch = parity::workspace_root()
        .join("tests/reference/_scratch")
        .join(stem);
    std::fs::create_dir_all(&scratch).expect("the scratch directory is writable");
    let expected_path = scratch.join("drc.expected.json");
    std::fs::write(&expected_path, &expected_text).expect("the scratch file is writable");

    let actual = parity::normalize_drc_json(port)
        .unwrap_or_else(|e| panic!("{stem}: the port's document does not parse: {e}"));
    let _ = std::fs::write(scratch.join("drc.actual.json"), &actual);
    parity::assert_text_parity(&actual, &expected_path);
}

/// The whole check for one stem, for the seven stems that are byte-parity.
fn check(stem: &str) {
    if !parity::require_java_dir() {
        return;
    }
    if !parity::require_reference(&reference_path(stem)) {
        return;
    }
    let row = row(stem);
    let mut reference = read_reference(stem);
    let port = port_json(&row, &reference);
    assert_normalised_parity(stem, &port, &mut reference);
}

// ---------------------------------------------------------------------------------------------
// One test per stem
//
// None of them is `#[cfg_attr(debug_assertions, ignore)]`, which the task brief suggested for the
// four largest by analogy with Plan 3's DSN suites: measured, all eleven tests here run in **3.0 s
// in a debug build** (0.4 s in release), because the DRC pass over even the largest board in the
// set is a fraction of the DSN read. Gating them would cost the default `cargo test` its coverage
// of the `.rules` path, the SES path and the two biggest boards for no wall-clock saving worth
// having.
// ---------------------------------------------------------------------------------------------

#[test]
fn drc_dev_board() {
    // 10 violations (2 `holeClearance`, 8 `track_dangling`), 4 unconnected entries,
    // qualityScore 902.078369140625.
    check("drc-dev-board");
}

#[test]
fn drc_bbd_mars_64() {
    // 96 violations (64 `holeClearance`, 12 `clearance`, 18 `via_dangling`, 2 `track_dangling`) —
    // the only stem in the set that produces a plain `clearance` entry — 3 unconnected,
    // qualityScore 828.276123046875.
    check("drc-bbd-mars-64");
}

#[test]
fn drc_issue593_rules() {
    // The `.rules` path (`-dr`): 0 violations, 74 unconnected entries, qualityScore 0.0.
    check("drc-issue593-rules");
}

#[test]
fn drc_issue593_ses() {
    // The session path: the same board with its `.ses` applied, which is what turns 0 violations
    // into 13 (1 `track_dangling`, 12 `via_dangling`) and 74 unconnected entries into 45.
    check("drc-issue593-ses");
}

#[test]
fn drc_issue753_cpu85() {
    // The largest board in the set: 107 violations (78 `holeClearance`, 16 `track_dangling`,
    // 13 `via_dangling`), 169 unconnected entries, qualityScore 331.1068115234375.
    check("drc-issue753-cpu85");
}

#[test]
fn drc_issue110_relay() {
    // 26 violations (4 `holeClearance`, 22 `via_dangling`), 44 unconnected entries, and a
    // qualityScore of exactly 0.0 — the other end of the range from the dev board's 902.
    check("drc-issue110-relay");
}

#[test]
fn drc_tutorial_board() {
    // The empty report: no violations, no unconnected entries, `schematicParity: []`. It is the
    // only stem that pins what the document looks like when every array is empty.
    check("drc-tutorial-board");
}

#[test]
fn natural_tone_preamp_is_the_reference_minus_three_dangling_tracks() {
    // Ruling S / quirk #146. The reference is the `-XX:hashCode=2` run, 115 violations; the port
    // emits 112. The three that differ are pinned by uuid, not by count: each is the `firstItem`
    // of a net entry whose connected group holds no `Pin`, so the port's ascending-id
    // `findRepresentativeItem` (ruling 3) picks it where that JVM run picked another item, and the
    // trace phase's dedup (`DesignRulesChecker.java:160`) then drops it. Everything else —
    // including the whole 44-entry `unconnectedItems` block — is byte-identical.
    if !parity::require_java_dir() {
        return;
    }
    let stem = "drc-natural-tone-preamp";
    if !parity::require_reference(&reference_path(stem)) {
        return;
    }
    /// The three `track_dangling` entries the JVM's hash choice adds and the port's does not.
    /// Task 7's `natural_tone_preamp_is_the_jvms_maximal_run_minus_three_dangling_tracks` pins the
    /// same trio at report level; this is the same fact at byte level.
    const EXTRA_DANGLING_UUIDS: [&str; 3] = ["1909", "1696", "1242"];

    let mut reference = read_reference(stem);
    assert_eq!(
        reference.violations.len(),
        115,
        "the committed reference must be the -XX:hashCode=2 run"
    );

    // Delete exactly those three entries, in place, and nothing else.
    let mut removed: Vec<String> = Vec::new();
    reference.violations.retain(|violation| {
        let drop = violation.kind == "track_dangling"
            && violation.items.len() == 1
            && EXTRA_DANGLING_UUIDS.contains(&violation.items[0].uuid.as_str());
        if drop {
            removed.push(violation.items[0].uuid.clone());
        }
        !drop
    });
    assert_eq!(
        removed,
        EXTRA_DANGLING_UUIDS.map(String::from),
        "all three pinned entries must be present, in the reference's own order"
    );
    assert_eq!(reference.violations.len(), 112);

    let row = row(stem);
    let port = port_json(&row, &reference);
    assert_normalised_parity(stem, &port, &mut reference);
}

// ---------------------------------------------------------------------------------------------
// Guards on the references themselves
// ---------------------------------------------------------------------------------------------

#[test]
fn references_are_from_the_head_jar() {
    // The guard against a regeneration with `scripts/gen-reference.sh`'s pinned 2.3.0 jar, which
    // spells every key in snake_case (plan-5 ruling 1). It would still be a valid DRC report; it
    // would not be *this* port's target, and `deny_unknown_fields` would fail with a confusing
    // message rather than this one.
    for row in rows() {
        let meta_path = parity::reference(&row.stem, "drc.meta.txt");
        if !parity::require_reference(&meta_path) {
            continue;
        }
        let meta = std::fs::read_to_string(&meta_path).expect("the meta file is readable");
        assert!(
            meta.contains("freerouting-current-executable.jar"),
            "{}: not generated from the clone's HEAD build:\n{meta}",
            row.stem
        );
        assert!(
            meta.contains("Freerouting 2.3.1-SNAPSHOT"),
            "{}: not generated from a 2.3.1-SNAPSHOT jar:\n{meta}",
            row.stem
        );
        assert!(
            meta.contains("-XX:hashCode=2"),
            "{}: not generated under the constant-hash mode:\n{meta}",
            row.stem
        );
    }
}

#[test]
fn every_reference_carries_a_quality_score_and_the_head_key_spelling() {
    // `qualityScore` is what driving the real CLI buys over a `DesignRulesChecker`-only driver
    // (plan-5 ruling 10): `generateReportJson` never sets it, so a reference taken that way would
    // have no key at all and the port's injected value would go untested.
    for row in rows() {
        if !parity::require_reference(&reference_path(&row.stem)) {
            continue;
        }
        let reference = read_reference(&row.stem);
        assert!(
            reference.quality_score.is_some(),
            "{}: no qualityScore",
            row.stem
        );
        assert_eq!(reference.schema, "https://schemas.kicad.org/drc.v1.json");
        assert_eq!(reference.kicad_version, "N/A");
        assert_eq!(reference.coordinate_units, "mm");
    }
}

#[test]
fn no_description_carries_a_comma_decimal() {
    // Plan-5 ruling 6: every `%.4f` in a violation description goes through `String.formatted`,
    // which uses the default FORMAT locale, so a JVM in a comma-decimal locale writes
    // `expected: 0,0500 mm`. `scripts/gen-drc-reference.sh` passes `-Duser.language=en
    // -Duser.country=US`; this is the assertion that says so. The port's formatter is
    // locale-free (`fr_dsn::format::double::java_format_fixed`), so a reference generated without
    // the flags would fail every stem with a confusing diff instead of failing here.
    for row in rows() {
        if !parity::require_reference(&reference_path(&row.stem)) {
            continue;
        }
        let reference = read_reference(&row.stem);
        for violation in reference
            .violations
            .iter()
            .chain(&reference.unconnected_items)
        {
            let bytes = violation.description.as_bytes();
            let comma_decimal = bytes
                .windows(3)
                .any(|w| w[1] == b',' && w[0].is_ascii_digit() && w[2].is_ascii_digit());
            assert!(
                !comma_decimal,
                "{}: comma decimal in {:?} — regenerate with -Duser.language=en \
                 -Duser.country=US",
                row.stem, violation.description
            );
        }
    }
}
