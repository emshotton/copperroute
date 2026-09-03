//! **Port-defect regression: the fanout target sort's tie-break.**
//!
//! `RoutingBoard.fanout:1002-1021` sorts the pin's unconnected set by the **squared** distance
//! from the pin centre to each candidate's bounding-box midpoint, and `List.sort` is TimSort —
//! **stable**. So on an exact distance tie the answer is decided entirely by the order the list
//! was *seeded* in, and that order is `new ArrayList<>(unconnectedSet)` over a `TreeSet<Item>`
//! whose `compareTo` is `item.id - id` (Item.java:95-103, `Item.getUnconnectedSet` at :676-690):
//! **descending id**, quirk **#44**. The port seeded a `BTreeSet` **ascending** and so broke every
//! tie the other way — a routing divergence on any board carrying a symmetric pad pair.
//!
//! # Why this fixture exists, and why it is hand-written
//!
//! The eight-stem `batch_parity` corpus is **byte-invisible** to the defect: not one corpus board
//! contains a fanout distance tie, so all eight stayed byte-identical to the jar with the bug in
//! place. It took an external board with a symmetric jumper to expose it. This fixture is the
//! smallest thing that reproduces the class:
//!
//! * `tests/data/p8-fanout-tie.dsn` — one component `JP1` whose four SMD pads sit on the corners
//!   of a square (±20000 × ±20000 DSN units about `150000,150000`), all on net `N1`. From the
//!   first pad fanout reaches, the two orthogonal neighbours are at **exactly** equal squared
//!   distance (`1.6e11` in board units) and the diagonal is further, so the tie-break alone picks
//!   the target.
//! * A second component `BLK1` — one large pad on its own net `N2` — sits between the two bottom
//!   pads. That is what makes the tie *decide the routed output* rather than merely mirror it: a
//!   perfectly symmetric board routes to the same SES either way. With the blocker, Java's choice
//!   (the higher id, the pad **up** the left edge) routes on `F.Cu` alone, and the ascending
//!   choice (the pad **across the blocker**) has to drop to `B.Cu` through two extra vias.
//!
//! # The reference
//!
//! `tests/data/p8-fanout-tie-jar.txt` is the HEAD jar's **own** answer, produced by
//! `java -jar <HEAD jar> -de <fixture> -do <ses>` with no other switch, transcribed one SES line
//! per `[jar-cli] ses|` row in the `p8t13-directed-*` style (`crates/fr-dsn/tests/data/`). The
//! fixture writes no `(host_cad …)`/`(host_version …)`, so quirk #92's
//! `parity::normalize_ses_head_tokens` has nothing to rewrite here and the comparison is raw
//! bytes.
//!
//! # Mutation-checked
//!
//! Removing the `.rev()` from `board_ext::routing_board_ext::sorted_unconnected_targets` makes
//! [`the_fanout_tie_break_matches_the_jar`] fail with two `(via VIA …)` scopes and a `B.Cu` detour
//! the jar does not have. Measured, both directions, before this file was committed.

use fr_dsn::{BoardReadResult, CoordinateTransform, DsnReadOptions};
use fr_router::pipeline::{
    NoopProgressSink, RouterBudget, RouterStop, prepare_board, run_pipeline,
};
use fr_settings::sources::{CliSettings, DsnFileSettings, EnvironmentVariablesSource};
use fr_settings::{HostEnvironment, SettingsInputs, SettingsSource, resolve_headless};

/// The fixture's stem, which is also `job.name` and therefore the SES header's design name
/// (`RoutingJob.setInputFromFile:432`/`:457`).
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

/// The `[jar-cli] ses|` rows of the transcript, rejoined into the jar's exact SES bytes.
///
/// The transcript also carries the byte count the jar wrote; it is asserted rather than ignored,
/// so a hand edit that drops or reflows a row cannot pass silently.
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

/// The jar's `-de <dsn> -do <ses>` flow with no other switch, the way
/// `tests/batch_parity.rs::route_stem` builds it: ruling AW's `resolve_headless` ladder and
/// `prepare_board`, then `run_pipeline` and the SES writer.
fn route(dsn: &std::path::Path) -> String {
    // One read: the same bytes feed `DsnFileSettings` (the priority-30 tier) and the parser.
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

    // The jar's argv, minus `-do`: this function writes the SES into a `Vec` rather than to a
    // path, and carrying an output path the test never creates would be a lie in the fixture. The
    // omission is settings-invisible — the CLI tier reads `-do` for the output *destination*, and
    // nothing below consults it — and the assertion is what proves it, because the reference on
    // the other side is the jar run **with** `-do`.
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
    // Ruling AI's budget, disabled on this side against the jar's javac-inlined live literals —
    // the same asymmetry `batch_parity` runs under. The fixture is 1.3 kB and neither literal can
    // bind on it, which is what keeps the comparison a wall-clock-independent one.
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
