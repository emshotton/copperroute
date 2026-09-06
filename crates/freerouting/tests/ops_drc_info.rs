#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use fr_drc::report::DrcJsonFlavor;
use freerouting::ops::drc::{DrcRequest, drc, report_date};
use freerouting::ops::info::{InfoRequest, info};
use freerouting::ops::load::{BoardSource, LoadRequest};

fn spike_dsn() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../benchmark/tests/data/spike/spike.dsn")
}

#[test]
fn the_date_is_iso_offset_date_time_not_iso_instant() {
    use std::time::{Duration, UNIX_EPOCH};
    let base = 1_756_800_000u64;
    assert_eq!(
        report_date(UNIX_EPOCH + Duration::new(base, 0)),
        "2025-09-02T08:00Z"
    );
    assert_eq!(
        report_date(UNIX_EPOCH + Duration::new(base, 120_000_000)),
        "2025-09-02T08:00:00.12Z"
    );
    assert_eq!(
        report_date(UNIX_EPOCH + Duration::new(base + 7, 0)),
        "2025-09-02T08:00:07Z"
    );
}

#[test]
fn info_summarises_the_spike_board() {
    let summary = info(&InfoRequest {
        load: LoadRequest::for_board(BoardSource::Path(spike_dsn())),
    })
    .unwrap();
    let text = summary.to_json_pretty();
    assert!(text.contains("\"layers\""));
    assert!(text.contains("\"statistics\""));
}

#[test]
fn drc_counts_the_dev_boards_violations_in_both_flavors() {
    if !parity::require_java_dir() {
        return;
    }
    let dsn = parity::fixture("Issue575-drc_dev-board_4_hole_clearance_violations.dsn");
    let request = DrcRequest {
        load: LoadRequest::for_board(BoardSource::Path(dsn.clone())),
        flavor: DrcJsonFlavor::KiCad,
        date: "2025-09-02T08:00Z".to_string(),
    };
    let outcome = drc(&request).unwrap();
    assert_eq!(outcome.violation_count, 8);
    assert!(
        outcome
            .json
            .contains("\"quality_score\": 906.2450561523438"),
        "{}",
        outcome.json
    );
    assert!(
        outcome
            .json
            .contains("\"freerouting_version\": \"Freerouting ")
    );
    assert!(outcome.json.contains(env!("CARGO_PKG_VERSION")));

    let head = DrcRequest {
        flavor: DrcJsonFlavor::FreeroutingHead,
        ..request
    };
    let outcome = drc(&head).unwrap();
    assert!(outcome.json.contains("\"qualityScore\""));
}

#[test]
fn a_rules_file_does_not_move_the_quality_score() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = std::env::temp_dir().join("fr-ops-drc");
    std::fs::create_dir_all(&dir).unwrap();
    let rules = dir.join("scoring.rules");
    std::fs::write(
        &rules,
        b"(rules PCB scoring\n  (autoroute_settings\n    (via_costs 999)\n  )\n)\n",
    )
    .unwrap();
    let dsn = parity::fixture("Issue575-drc_dev-board_4_hole_clearance_violations.dsn");
    let plain = drc(&DrcRequest {
        load: LoadRequest::for_board(BoardSource::Path(dsn.clone())),
        flavor: DrcJsonFlavor::KiCad,
        date: "d".to_string(),
    })
    .unwrap();
    let mut load = LoadRequest::for_board(BoardSource::Path(dsn));
    load.rules = Some(rules);
    let with_rules = drc(&DrcRequest {
        load,
        flavor: DrcJsonFlavor::KiCad,
        date: "d".to_string(),
    })
    .unwrap();
    assert_eq!(plain.report.quality_score, with_rules.report.quality_score);
}
