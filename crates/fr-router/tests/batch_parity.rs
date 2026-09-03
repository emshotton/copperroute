//! Plan 7 Task 16: the port's **whole-board** run against the HEAD jar's, on eight boards.
//!
//! # What the references are
//!
//! `tests/reference/<stem>/batch.ses` is the clone's HEAD build's **verbatim** SES for a
//! whole-board `java -jar <jar> -de <dsn> -do <ses> -mp <n>` run, and `batch.passes.jsonl` its
//! per-pass `PassRecord` tuples. Both are written by `scripts/gen-batch-reference.sh` through
//! `scripts/differential/java/P7T9.java` — the same driver `scripts/differential/run.sh p7t9`
//! diffs live against the same Rust code this file runs, so the reference and the differential
//! can never describe different runs. `--verify-driver` is what makes the indirection safe: it
//! runs the *bare jar* on the same `argv` and requires the two SES files to be byte-identical
//! (measured: identical on all eight stems), and its verdict is committed into each stem's
//! `batch.meta.txt`. [`the_driver_matches_the_bare_jar`] asserts it is there and clean.
//!
//! # Why the run goes through `prepare_board` and `resolve_headless`
//!
//! Controller ruling AW. The jar's real `-de/-do` flow loads through
//! `management/HeadlessBoardManager`, whose `applyCopperToEdgeClearanceOverride` and
//! `applyHoleClearanceOverride` **mutate the board on every load** — on 15 of the 16 corpus
//! boards, measured by Task 15b — and resolves settings through the two-merge ladder of
//! `Freerouting.java:125-146` + `RoutingJobScheduler.java:103-186`. A run that loaded with
//! `fr_dsn::read_board` and `DefaultSettings` alone would route a different board and fail every
//! rung with a false XDIFF pointing at the router. So [`route_stem`] is
//! [`fr_settings::resolve_headless`] (that ladder, one linear pass, byte-pinned against the JVM by
//! Plan 4's `p4t1`) followed by [`fr_router::pipeline::prepare_board`] (`:746-747`), and only then
//! [`run_pipeline`].
//!
//! # The acceptance ladder (ruling 1, recorded in `crates/fr-router/README.md`)
//!
//! Per stem, in order:
//!
//! * **(a)** the pass count and every `PassRecord` tuple identical — [`rung_a_pass_records`];
//! * **(b)** the item set after every pass identical — [`rung_b_item_sets`];
//! * **(c)** byte-identical SES — [`rung_c_ses_bytes`].
//!
//! **Rung (b)'s committed half is counts, not sets**, and `crates/fr-router/README.md`'s
//! acceptance section says so beside the table: the reference carries no per-pass item *sets* to
//! compare against, so [`rung_b_item_sets`] compares the per-pass via and trace counts and the
//! final SES's scope count. The final item set is pinned exactly anyway — by rung (c), since a
//! byte-identical SES enumerates the same items with the same geometry — and the full per-pass
//! statement (every id, layer, half width, polyline and corner list after every pass) is
//! `sweep-p7t9.sh`'s `batch-router` rows, which diff both sides' complete `[board]` dumps.
//!
//! # Two aggregate climbs, not one test per stem
//!
//! The plan asks for "one test per stem". Delivered instead:
//! [`the_ci_stems_climb_the_whole_ladder`] over the four CI stems and
//! [`the_slow_stems_climb_the_whole_ladder`] over all eight. **The cost is that a failure on the
//! first stem of a lane masks the rest of that lane** until it is fixed. The shape was kept
//! because ruling AM's lanes are a property of the *set* (`FR_SLOW_PARITY` gates one whole lane
//! and [`parity::require_java_dir`] skips both), because a whole-board stem costs seconds to
//! minutes and the eight are run as a ladder rather than individually, and because [`STEMS`] is
//! the one table the fixture-file cross-check and the two provenance tests already iterate.
//! Splitting [`climb_all`] into eight `#[test]`s is mechanical if per-stem reporting is later
//! worth more. Recorded in `crates/fr-router/README.md` too, because it is a deviation.
//!
//! **Measured result: all eight stems reach (a), (b) and (c).** Ruling AM's escape hatch — a stem
//! that reaches (a)+(b) but not (c) becomes an `XDIFF` README row carrying the first differing
//! byte and a normalised digest — is therefore unused, and [`every_stem_reaches_rung_c`] is what
//! stops it being quietly re-entered.
//!
//! # The one normalisation, and why it is not a tolerance
//!
//! [`parity::normalize_ses_head_tokens`] rewrites four `(parser …)` keyword literals on the
//! **reference** side. HEAD camelCased them (`(hostCad `, `(hostVersion `, `(stringQuote `,
//! `(writeResolution `) while leaving its own lexer recognising only the snake_case tokens, so it
//! writes Specctra it cannot read back; Plan 3 ruling 1 therefore pins the port's writer to the
//! 2.3.0 spelling and `tests/reference/README.md` forbids regenerating the DSN/SES references
//! from HEAD. `batch.ses` is the one file in this tree written by HEAD's `SesWriter`, so it
//! carries HEAD's spelling. The set is closed, enumerated and documented as quirk **#92**;
//! everything else is compared byte for byte. See the normaliser's own doc comment.
//!
//! # What runs in a debug build
//!
//! Ruling AM as amended by scan ruling 13: **eight stems, four in CI**
//! (`router-rpi-splitter`, `router-j2-reference`, `router-ecc83-input`, `router-empty-board`),
//! four `#[cfg_attr(debug_assertions, ignore)]` + `FR_SLOW_PARITY=1`
//! (`router-dac2020-bm01`, `router-tutorial-board`, `router-fanout-bm11`,
//! `router-strict-drc-cnh`). The lane per stem is column `ci` of [`STEMS`], and it mirrors
//! `tests/reference/router-fixtures.txt`'s own table.

use std::collections::BTreeSet;

use fr_dsn::{BoardReadResult, CoordinateTransform, DsnReadOptions};
use fr_router::pipeline::{
    NoopProgressSink, RouterBudget, RouterStop, prepare_board, run_pipeline,
};
use fr_settings::sources::{CliSettings, DsnFileSettings, EnvironmentVariablesSource};
use fr_settings::{HostEnvironment, SettingsInputs, SettingsSource, resolve_headless};
use parity::BatchPassDoc;

/// One batch row of `tests/reference/router-fixtures.txt`, columns 1, 2, 5, 6 and 7 plus the CI
/// lane. Duplicated here rather than parsed out of the file because a test that reads its own
/// expectations from a file it also drives can agree with itself while disagreeing with the
/// reference; the shape is `reference_parity.rs`'s `STEMS`.
struct Stem {
    /// The reference directory under `tests/reference/`.
    name: &'static str,
    /// The DSN, relative to the Java checkout.
    dsn: &'static str,
    /// `settings.maxPasses`, i.e. `-mp`.
    max_passes: i32,
    /// `--router.fanout.enabled`.
    fanout: bool,
    /// `--router.optimizer.enabled`.
    optimizer: bool,
    /// Whether the stem runs under a plain `cargo test --workspace`.
    ci: bool,
}

/// The eight stems, in `router-fixtures.txt` order.
const STEMS: &[Stem] = &[
    Stem {
        name: "router-rpi-splitter",
        dsn: "fixtures/Issue143-rpi_splitter.dsn",
        max_passes: 8,
        fanout: true,
        optimizer: true,
        ci: true,
    },
    Stem {
        name: "router-dac2020-bm01",
        dsn: "fixtures/Issue508-DAC2020_bm01.dsn",
        max_passes: 2,
        fanout: true,
        optimizer: true,
        ci: false,
    },
    Stem {
        name: "router-j2-reference",
        dsn: "fixtures/Issue026-J2_reference.dsn",
        max_passes: 99,
        fanout: true,
        optimizer: true,
        ci: true,
    },
    Stem {
        name: "router-tutorial-board",
        dsn: "examples/tutorial_board/tutorial_board.dsn",
        max_passes: 8,
        fanout: true,
        optimizer: true,
        ci: false,
    },
    Stem {
        name: "router-ecc83-input",
        dsn: "fixtures/Issue649-kicad_ecc83-pp_input_board_v1.dsn",
        max_passes: 8,
        fanout: true,
        optimizer: true,
        ci: true,
    },
    Stem {
        name: "router-fanout-bm11",
        dsn: "fixtures/Issue730-DAC2020_bm11.dsn",
        max_passes: 2,
        fanout: true,
        optimizer: false,
        ci: false,
    },
    Stem {
        name: "router-strict-drc-cnh",
        dsn: "fixtures/Issue555-CNH_Functional_Tester_1.dsn",
        max_passes: 2,
        fanout: true,
        optimizer: true,
        ci: false,
    },
    Stem {
        name: "router-empty-board",
        dsn: "fixtures/empty_board.dsn",
        max_passes: 1,
        fanout: false,
        optimizer: false,
        ci: true,
    },
];

/// What one whole-board run produces, i.e. the three rungs' raw material.
struct BatchRun {
    /// Rung (a): the routing stage's per-pass tuples.
    passes: Vec<BatchPassDoc>,
    /// Rung (b): every item id on the final board.
    items: BTreeSet<u32>,
    /// Rung (c): the SES the port's writer produced.
    ses: String,
}

/// The jar's `-de <dsn> -do <ses> -mp <n>` flow, port side, exactly as `P7T9 … batch` runs it.
fn route_stem(stem: &Stem) -> BatchRun {
    let dsn = parity::java_dir().join(stem.dsn);
    let bytes =
        std::fs::read(&dsn).unwrap_or_else(|e| panic!("cannot read {}: {e}", dsn.display()));
    let file_name = dsn
        .file_name()
        .expect("a file name")
        .to_string_lossy()
        .into_owned();
    // `RoutingJob.setInputFromFile:432` + `:457` — `job.name` is the base name without the
    // extension, and `SesWriter.write(board, out, job.name)` is what the SES header carries.
    let design_name = file_name
        .rsplit_once('.')
        .map_or_else(|| file_name.clone(), |(base, _)| base.to_string());

    let (mut board, transform) = read_board(&dsn, &file_name);

    // `P7T9.batchArgv` — the same five switches the bare jar is given, so `CliSettings`
    // (priority 60) sees the same input on both sides.
    let argv = vec![
        "-de".to_string(),
        dsn.display().to_string(),
        "-do".to_string(),
        dsn.display().to_string().replace(".dsn", ".ses"),
        "-mp".to_string(),
        stem.max_passes.to_string(),
        format!("--router.fanout.enabled={}", stem.fanout),
        format!("--router.optimizer.enabled={}", stem.optimizer),
    ];
    let dsn_source = DsnFileSettings::new(&bytes[..], &file_name);
    let env_map: std::collections::BTreeMap<String, String> = std::env::vars().collect();
    let env_source = EnvironmentVariablesSource::new(&env_map);
    let cli_source = CliSettings::new(&argv);
    let inputs = SettingsInputs {
        json_file: None,
        dsn: dsn_source.get_settings(),
        // No `-dr`, and no corpus stem has an adjacent `<design>.rules`, so
        // `RoutingJobScheduler.java:154-160` and `:173-184` are both skipped on the Java side too.
        cli_rules: None,
        scheduler_rules: None,
        env: env_source.get_settings(),
        cli: cli_source.get_settings(),
    };
    let settings = resolve_headless(&inputs, Some(&board), &HostEnvironment::detect());
    // `HeadlessBoardManager.java:746-747`, ruling AW.
    prepare_board(&mut board, &settings);

    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    // Ruling AI's budget, disabled on this side against the jar's javac-inlined live 1000 ms
    // `TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP` — see `scripts/gen-batch-reference.sh`'s header for why
    // the Java side cannot be configured to match, and why a byte-identical SES under that
    // asymmetry is the stronger evidence.
    let result = run_pipeline(
        &mut board,
        &settings,
        &stop,
        RouterBudget::disabled(),
        &mut sink,
    )
    .expect("every batch stem has a routable signal layer");

    let passes = result
        .per_pass
        .iter()
        .map(|record| BatchPassDoc {
            pass: record.pass,
            score: record.score,
            incompletes: record.incomplete_count,
            violations: record.clearance_violations,
            vias: record.via_count,
            traces: record.trace_count,
        })
        .collect();
    let items = board.items.keys().map(|id| id.0).collect();

    let mut ses = Vec::new();
    fr_dsn::ses_writer::write(&board, &transform, &mut ses, &design_name)
        .expect("the SES writer cannot fail on a Vec");
    BatchRun {
        passes,
        items,
        ses: String::from_utf8(ses).expect("the SES writer emits UTF-8"),
    }
}

/// `fr_dsn::read_board` plus the coordinate transform `fr_dsn::ses_writer::write` needs (Plan 3
/// ruling A keeps it in `fr-dsn`, so it comes back on the read result rather than off the board).
fn read_board(dsn: &std::path::Path, design_name: &str) -> (fr_board::Board, CoordinateTransform) {
    let file = std::fs::File::open(dsn).unwrap_or_else(|e| panic!("cannot open {dsn:?}: {e}"));
    match fr_dsn::read_board(file, None, Some(design_name), &DsnReadOptions::default()) {
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
            *board.expect("the batch stems all produce a board"),
            coordinate_transform.expect("the batch stems all produce a coordinate transform"),
        ),
        other => panic!("{design_name} did not read: {other:?}"),
    }
}

/// Rung (a): the pass count and every `PassRecord` tuple.
fn rung_a_pass_records(stem: &Stem, run: &BatchRun, reference: &[BatchPassDoc]) {
    assert_eq!(
        run.passes.len(),
        reference.len(),
        "{}: rung (a) — pass count {} against the jar's {}",
        stem.name,
        run.passes.len(),
        reference.len()
    );
    for (actual, expected) in run.passes.iter().zip(reference) {
        assert_eq!(
            actual, expected,
            "{}: rung (a) — pass {} tuple differs",
            stem.name, expected.pass
        );
    }
}

/// Rung (b): the item set after every pass — **at count strength**; see the module doc.
///
/// The per-pass half is the via and trace counts the tuples carry, which rung (a) has already
/// compared element by element; asserting them again here is deliberate, because the two rungs
/// fail for different reasons and a reader of the failure needs to know which one broke. The
/// final half is the SES's own scope count on both sides. The collected [`BatchRun::items`] id
/// set is **not** compared against anything, because the committed reference carries no item ids
/// — a SES names no id — which is exactly why this rung is counts here and full item-by-item
/// dumps in `sweep-p7t9.sh`. Rung (c) is what pins the final item set exactly.
fn rung_b_item_sets(stem: &Stem, run: &BatchRun, reference: &[BatchPassDoc], reference_ses: &str) {
    for (actual, expected) in run.passes.iter().zip(reference) {
        assert_eq!(
            (actual.vias, actual.traces),
            (expected.vias, expected.traces),
            "{}: rung (b) — item counts after pass {} differ",
            stem.name,
            expected.pass
        );
    }
    let reference_wires = count_wire_scopes(reference_ses);
    let actual_wires = count_wire_scopes(&run.ses);
    assert_eq!(
        actual_wires, reference_wires,
        "{}: rung (b) — the final SES names {actual_wires} wires against the jar's {reference_wires}",
        stem.name
    );
    assert!(
        !run.items.is_empty() || stem.name == "router-empty-board",
        "{}: rung (b) — the final board has no items at all",
        stem.name
    );
}

/// `(wire ` scope openings in an SES — the routed traces and vias the session file enumerates.
fn count_wire_scopes(ses: &str) -> usize {
    ses.lines()
        .filter(|line| {
            let t = line.trim_start();
            t == "(wire" || t.starts_with("(wire ") || t.starts_with("(via ")
        })
        .count()
}

/// Rung (c): byte-identical SES, modulo the four enumerated quirk-#92 keyword literals.
fn rung_c_ses_bytes(stem: &Stem, run: &BatchRun, reference_ses: &str) {
    let expected = parity::normalize_ses_head_tokens(reference_ses);
    if run.ses == expected {
        return;
    }
    let first = run
        .ses
        .as_bytes()
        .iter()
        .zip(expected.as_bytes())
        .position(|(a, b)| a != b)
        .unwrap_or_else(|| run.ses.len().min(expected.len()));
    let context = |s: &str| {
        let start = s[..first.min(s.len())]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        let end = s[start..].find('\n').map_or(s.len(), |index| start + index);
        s[start..end].to_string()
    };
    panic!(
        "{}: rung (c) — SES differs at byte {first}\n  port: {}\n  jar : {}",
        stem.name,
        context(&run.ses),
        context(&expected)
    );
}

/// Reads a stem's two reference files. `None` is "not generated on this checkout", which
/// [`parity::require_reference`] has already announced.
fn read_reference(stem: &Stem) -> Option<(Vec<BatchPassDoc>, String)> {
    let ses_path = parity::reference(stem.name, "batch.ses");
    let passes_path = parity::reference(stem.name, "batch.passes.jsonl");
    if !parity::require_reference(&ses_path) || !parity::require_reference(&passes_path) {
        return None;
    }
    let passes = parity::parse_batch_passes(
        &std::fs::read_to_string(&passes_path).expect("the passes reference is readable"),
    )
    .expect("the passes reference parses");
    let ses = std::fs::read_to_string(&ses_path).expect("the SES reference is readable");
    Some((passes, ses))
}

/// The whole ladder for one stem.
fn climb(stem: &Stem) {
    let Some((reference_passes, reference_ses)) = read_reference(stem) else {
        return;
    };
    let run = route_stem(stem);
    rung_a_pass_records(stem, &run, &reference_passes);
    rung_b_item_sets(stem, &run, &reference_passes, &reference_ses);
    rung_c_ses_bytes(stem, &run, &reference_ses);
}

fn climb_all(ci_only: bool) {
    if !parity::require_java_dir() {
        return;
    }
    for stem in STEMS.iter().filter(|s| !ci_only || s.ci) {
        climb(stem);
    }
}

// =================================================================================================
// The ladder
// =================================================================================================

#[test]
fn the_ci_stems_climb_the_whole_ladder() {
    climb_all(true);
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "slow in debug; run with FR_SLOW_PARITY=1 --release"
)]
fn the_slow_stems_climb_the_whole_ladder() {
    if std::env::var_os("FR_SLOW_PARITY").is_none() {
        return;
    }
    climb_all(false);
}

/// Ruling AM's escape hatch is unused, and this is what stops it being quietly re-entered: rung
/// (c) is asserted for **every** stem, CI or slow, so a future change that demotes one to `XDIFF`
/// has to say so in the README's acceptance table rather than in a passing test.
#[test]
fn every_stem_reaches_rung_c() {
    let readme = std::fs::read_to_string(
        parity::workspace_root()
            .join("crates")
            .join("fr-router")
            .join("README.md"),
    )
    .expect("crates/fr-router/README.md");
    for stem in STEMS {
        let row = readme
            .lines()
            .find(|line| line.contains(stem.name) && line.starts_with('|'))
            .unwrap_or_else(|| panic!("{} has no acceptance row in the README", stem.name));
        assert!(
            !row.contains("XDIFF"),
            "{} is an XDIFF row in the README but no test records the divergence",
            stem.name
        );
    }
}

// =================================================================================================
// The references' provenance
// =================================================================================================

/// The clone's HEAD build is the parity jar for the whole of Plans 6 and 7, and a reference
/// generated from any other build would be a different algorithm rather than an older one.
///
/// **Not a version string:** HEAD's `META-INF/MANIFEST.MF` carries
/// `Implementation-Version: unspecified`, so `Build-Revision` is the only field that pins which
/// build produced these bytes.
///
/// # Lane-aware since Plan 9 (ruling BT)
///
/// A reference family may sit in the **port** lane from Plan 9 on: the first fix that *moves* a
/// family regenerates it with `--from-port`, and from then on its bytes are the port's rather
/// than the jar's. The meta file says which lane it is in on its own `lane` line, and a file with
/// no such line is a jar-lane file — that is what every pre-Plan-9 meta is. This test asserts the
/// invariant **of the lane the file declares**: the jar's build identity in the jar lane, and the
/// port sha plus the Plan 9 task that cut it in the port lane. Asserting the jar's identity over a
/// port-cut file would only be asserting that nobody had switched lanes, which is not a property
/// anything wants.
///
/// **This family is in the port lane from Plan 9 Task 2**: R1 (#293) reorders the work list and
/// R2 (#294) stops the fanout fallback emitting sub-minimum traces, so every routed stem's SES
/// moved and the B family regenerated for cause.
#[test]
fn references_are_from_the_head_jar() {
    let mut revisions = BTreeSet::new();
    for stem in STEMS {
        let meta_path = parity::reference(stem.name, "batch.meta.txt");
        if !parity::require_reference(&meta_path) {
            continue;
        }
        let meta = std::fs::read_to_string(&meta_path).expect("batch.meta.txt is readable");
        if declared_lane(&meta).starts_with("port") {
            assert_port_lane_provenance(&meta, stem.name);
            continue;
        }
        assert!(
            meta.contains("freerouting-current-executable.jar"),
            "{}: batch.meta.txt does not name the HEAD jar",
            stem.name
        );
        let revision = meta
            .lines()
            .find_map(|line| line.strip_prefix("jar revision "))
            .map(str::trim)
            .unwrap_or_else(|| panic!("{}: batch.meta.txt has no `jar revision`", stem.name));
        assert_eq!(
            revision.len(),
            40,
            "{}: `jar revision` is not a 40-character git revision: {revision}",
            stem.name
        );
        revisions.insert(revision.to_string());
    }
    assert!(
        revisions.len() <= 1,
        "the eight references come from {} different jar builds: {revisions:?}",
        revisions.len()
    );
}

/// `--verify-driver`'s verdict, committed into each stem's `batch.meta.txt`.
///
/// Controller answer 1: a bare-jar difference is informational only when the `optChangedArea`
/// budget actually fired, and a difference with **zero** recorded trips is a failure, because the
/// probe is then not the jar. Measured on this corpus: identical on all eight, so no stem carries
/// a trip count at all.
#[test]
fn the_driver_matches_the_bare_jar() {
    for stem in STEMS {
        let meta_path = parity::reference(stem.name, "batch.meta.txt");
        if !parity::require_reference(&meta_path) {
            continue;
        }
        let meta = std::fs::read_to_string(&meta_path).expect("batch.meta.txt is readable");
        let verdict = meta
            .lines()
            .find_map(|line| line.strip_prefix("bare-jar "))
            .map(str::trim)
            .unwrap_or_else(|| {
                panic!(
                    "{}: batch.meta.txt records no --verify-driver verdict; run \
                     scripts/gen-batch-reference.sh --verify-driver {}",
                    stem.name, stem.name
                )
            });
        assert!(
            verdict.starts_with("identical"),
            "{}: --verify-driver verdict is `{verdict}`, not `identical`",
            stem.name
        );
    }
}

/// The stem table in this file and the batch rows of `tests/reference/router-fixtures.txt` are two
/// copies of the same eight rows; this is what keeps them from drifting.
#[test]
fn the_stem_table_matches_the_fixture_file() {
    let fixtures = std::fs::read_to_string(
        parity::workspace_root()
            .join("tests")
            .join("reference")
            .join("router-fixtures.txt"),
    )
    .expect("tests/reference/router-fixtures.txt");
    let rows: Vec<Vec<&str>> = fixtures
        .lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .map(|line| line.split('|').collect())
        .filter(|fields: &Vec<&str>| fields.len() >= 7 && fields[4] != "-")
        .collect();
    assert_eq!(
        rows.len(),
        STEMS.len(),
        "the fixture file has {} batch rows against this file's {}",
        rows.len(),
        STEMS.len()
    );
    for (row, stem) in rows.iter().zip(STEMS) {
        assert_eq!(row[0], stem.name, "stem name");
        assert_eq!(row[1], stem.dsn, "{}: dsn", stem.name);
        assert_eq!(
            row[4],
            stem.max_passes.to_string(),
            "{}: max_passes",
            stem.name
        );
        assert_eq!(
            row[5],
            if stem.fanout { "on" } else { "off" },
            "{}: fanout",
            stem.name
        );
        assert_eq!(
            row[6],
            if stem.optimizer { "on" } else { "off" },
            "{}: optimizer",
            stem.name
        );
    }
}

/// The normaliser touches exactly the four keyword literals it names, and nothing else — in
/// particular it must not rewrite a `(hostCad ` that appears *inside* a quoted string value, which
/// is why it anchors on the line's first non-blank characters.
#[test]
fn the_ses_normaliser_touches_only_the_parser_keywords() {
    let input = "(session \"x\"\n  (parser\n    (hostCad CadSoft)\n    \
                 (hostVersion \"a (hostCad b) c\")\n  )\n  (wire (path F.Cu 100 1 2 3 4))\n)\n";
    let expected = "(session \"x\"\n  (parser\n    (host_cad CadSoft)\n    \
                    (host_version \"a (hostCad b) c\")\n  )\n  \
                    (wire (path F.Cu 100 1 2 3 4))\n)\n";
    assert_eq!(parity::normalize_ses_head_tokens(input), expected);
    // Idempotent, and a no-op on an SES the port itself wrote.
    assert_eq!(parity::normalize_ses_head_tokens(expected), expected);
}

/// The lane a reference meta file declares (ruling BT). A file with no `lane` line predates the
/// Plan 9 lane switch and is a jar-lane file.
fn declared_lane(meta: &str) -> &str {
    meta.lines()
        .find_map(|line| line.strip_prefix("lane "))
        .map(str::trim)
        .unwrap_or("jar")
}

/// The port lane's own provenance: the sha of the build that wrote the file and the Plan 9 task
/// it was cut at, both written by the generator. They are what makes a port-cut reference
/// traceable to a commit, exactly as `jar revision` does in the other lane.
fn assert_port_lane_provenance(meta: &str, what: &str) {
    let sha = meta
        .lines()
        .find_map(|line| line.strip_prefix("port sha "))
        .map(str::trim)
        .unwrap_or_else(|| panic!("{what}: a port-lane meta with no `port sha` line:\n{meta}"));
    assert!(
        sha.len() >= 12 && sha.chars().all(|c| c.is_ascii_hexdigit()),
        "{what}: `port sha` is not a git sha: {sha}"
    );
    let task = meta
        .lines()
        .find_map(|line| line.strip_prefix("plan 9 task "))
        .map(str::trim)
        .unwrap_or_else(|| panic!("{what}: a port-lane meta with no `plan 9 task` line:\n{meta}"));
    assert!(
        task.starts_with('T') && task[1..].chars().all(|c| c.is_ascii_digit()),
        "{what}: `plan 9 task` is not a task id: {task}"
    );
}
