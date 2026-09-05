use fr_dsn::{BoardReadResult, CoordinateTransform, DsnReadOptions};
use fr_router::pipeline::{
    NoopProgressSink, RouterBudget, RouterStop, prepare_board, run_pipeline,
};
use fr_settings::sources::{CliSettings, DsnFileSettings, EnvironmentVariablesSource};
use fr_settings::{HostEnvironment, SettingsInputs, SettingsSource, resolve_headless};

const STEM: &str = "p8-fanout-tie";

#[test]
fn the_fanout_tie_break_matches_the_jar() {
    let root = parity::workspace_root();
    let dsn = root.join(format!("crates/fr-router/tests/data/{STEM}.dsn"));
    let transcript = root.join(format!("crates/fr-router/tests/data/{STEM}-jar.txt"));

    let expected = jar_ses(&transcript);
    let actual = route(&dsn);

    assert_eq!(
        actual, expected,
        "the port's SES must be byte-identical to the HEAD jar's on the tie fixture; \
         a diff here means the fanout target sort is no longer seeded in quirk #44's \
         descending id order (see `sorted_unconnected_targets`)"
    );
}

fn jar_ses(transcript: &std::path::Path) -> String {
    let text = std::fs::read_to_string(transcript)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", transcript.display()));
    let rows: Vec<&str> = text
        .lines()
        .filter_map(|line| line.strip_prefix("[jar-cli] ses|"))
        .collect();
    assert!(
        !rows.is_empty(),
        "the transcript carries the jar's SES rows"
    );
    let ses = rows.join("\n");
    let declared: usize = text
        .lines()
        .find_map(|line| line.strip_prefix("[jar-cli] ses bytes="))
        .expect("the transcript declares the jar's SES byte count")
        .trim()
        .parse()
        .expect("a byte count");
    assert_eq!(
        ses.len(),
        declared,
        "the transcript's rows must rebuild exactly the bytes it says the jar wrote"
    );
    ses
}

fn route(dsn: &std::path::Path) -> String {
    let bytes = std::fs::read(dsn).unwrap_or_else(|e| panic!("cannot read {}: {e}", dsn.display()));
    let file_name = format!("{STEM}.dsn");

    let (mut board, transform): (fr_board::Board, CoordinateTransform) = match fr_dsn::read_board(
        std::io::Cursor::new(&bytes[..]),
        None,
        Some(&file_name),
        &DsnReadOptions::default(),
    ) {
        BoardReadResult::Success {
            board,
            coordinate_transform,
            ..
        } => (
            *board.expect("the fixture produces a board"),
            coordinate_transform.expect("the fixture produces a coordinate transform"),
        ),
        other => panic!("{STEM} did not read: {other:?}"),
    };

    let argv = vec!["-de".to_string(), dsn.display().to_string()];
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
    run_pipeline(
        &mut board,
        &settings,
        &stop,
        RouterBudget::disabled(),
        &mut sink,
    )
    .expect("the fixture has a routable signal layer");

    let mut ses = Vec::new();
    fr_dsn::ses_writer::write(&board, &transform, &mut ses, STEM)
        .expect("the SES writer cannot fail on a Vec");
    String::from_utf8(ses).expect("the SES writer emits UTF-8")
}
