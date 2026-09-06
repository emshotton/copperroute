//! Splitting [`climb_all`] into eight `#[test]`s is mechanical if per-stem reporting is later
//! four `#[cfg_attr(debug_assertions, ignore)]` + `FR_SLOW_PARITY=1`
use std::collections::BTreeSet;

use fr_dsn::{BoardReadResult, CoordinateTransform, DsnReadOptions};
use fr_router::pipeline::{
    NoopProgressSink, RouterBudget, RouterStop, prepare_board, run_pipeline,
};
use fr_settings::sources::{CliSettings, DsnFileSettings, EnvironmentVariablesSource};
use fr_settings::{HostEnvironment, SettingsInputs, SettingsSource, resolve_headless};
use parity::BatchPassDoc;

struct Stem {
    name: &'static str,
    dsn: &'static str,
    max_passes: i32,
    fanout: bool,
    optimizer: bool,
    ci: bool,
}

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

struct BatchRun {
    passes: Vec<BatchPassDoc>,
    items: BTreeSet<u32>,
    ses: String,
}

fn route_stem(stem: &Stem) -> BatchRun {
    let dsn = parity::reference_dir().join(stem.dsn);
    let bytes =
        std::fs::read(&dsn).unwrap_or_else(|e| panic!("cannot read {}: {e}", dsn.display()));
    let file_name = dsn
        .file_name()
        .expect("a file name")
        .to_string_lossy()
        .into_owned();
    let design_name = file_name
        .rsplit_once('.')
        .map_or_else(|| file_name.clone(), |(base, _)| base.to_string());

    let (mut board, transform) = read_board(&dsn, &file_name);

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
        cli_rules: None,
        scheduler_rules: None,
        env: env_source.get_settings(),
        cli: cli_source.get_settings(),
    };
    let settings = resolve_headless(&inputs, Some(&board), &HostEnvironment::detect());
    prepare_board(&mut board, &settings);

    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
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

fn count_wire_scopes(ses: &str) -> usize {
    ses.lines()
        .filter(|line| {
            let t = line.trim_start();
            t == "(wire" || t.starts_with("(wire ") || t.starts_with("(via ")
        })
        .count()
}

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

fn climb(stem: &Stem) {
    let Some((reference_passes, reference_ses)) = read_reference(stem) else {
        return;
    };
    let run = route_stem(stem);
    if parity::regolden_label().is_some() {
        std::fs::write(parity::reference(stem.name, "batch.ses"), &run.ses)
            .expect("the batch.ses reference is writable");
        parity::write_batch_passes(
            &parity::reference(stem.name, "batch.passes.jsonl"),
            &run.passes,
        );
        return;
    }
    rung_a_pass_records(stem, &run, &reference_passes);
    rung_b_item_sets(stem, &run, &reference_passes, &reference_ses);
    rung_c_ses_bytes(stem, &run, &reference_ses);
}

fn climb_all(ci_only: bool) {
    if !parity::require_reference_dir() {
        return;
    }
    for stem in STEMS.iter().filter(|s| !ci_only || s.ci) {
        climb(stem);
    }
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

#[test]
fn references_are_from_the_head_jar() {
    let mut revisions = BTreeSet::new();
    for stem in STEMS {
        let meta_path = parity::reference(stem.name, "batch.meta.txt");
        if !parity::require_reference(&meta_path) {
            continue;
        }
        let meta = std::fs::read_to_string(&meta_path).expect("batch.meta.txt is readable");
        if parity::declared_lane(&meta).starts_with("port") {
            parity::assert_port_lane_provenance(&meta, stem.name);
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

#[test]
fn the_ses_normaliser_touches_only_the_parser_keywords() {
    let input = "(session \"x\"\n  (parser\n    (hostCad CadSoft)\n    \
                 (hostVersion \"a (hostCad b) c\")\n  )\n  (wire (path F.Cu 100 1 2 3 4))\n)\n";
    let expected = "(session \"x\"\n  (parser\n    (host_cad CadSoft)\n    \
                    (host_version \"a (hostCad b) c\")\n  )\n  \
                    (wire (path F.Cu 100 1 2 3 4))\n)\n";
    assert_eq!(parity::normalize_ses_head_tokens(input), expected);
    assert_eq!(parity::normalize_ses_head_tokens(expected), expected);
}
