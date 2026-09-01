//! Plan 7 Task 15b: the corpus replay of `probes/P7T15bProbe.java`.
//!
//! The probe loads each of the sixteen parity-corpus DSNs through the **real**
//! `HeadlessBoardManager.loadFromSpecctraDsn`, nine times per board, with
//! `router.copper_to_edge_clearance_um` / `router.hole_clearance_um` forced to a different pair
//! each time, and prints the resulting clearance-matrix, outline-class, `holeClearance` and
//! hole-keepout state. Its stdout is committed verbatim as
//! `tests/data/p7t15b-clearance-overrides.txt`.
//!
//! This test re-derives every one of those blocks from the port: it loads the same DSN once
//! through `fr_dsn::read_board` (which answers the pristine board, i.e. the probe's variant A),
//! clones it per variant, runs [`fr_router::pipeline::prepare_board`] with the variant's two
//! settings values, and renders the probe's own line format from the resulting `Board`. A
//! mismatch prints the two blocks side by side.
//!
//! # What each variant proves
//!
//! * **A** — the port's loader and Java's agree on the pristine board, so every later diff is
//!   the override's and not the reader's.
//! * **B** / **D** — `applyCopperToEdgeClearanceOverride` at the real merged 500 µm: the
//!   `board_edge` class, its row *and* column on every layer, and the re-pointed outline. It
//!   fires on 15 of the 16 boards; `router-rpi-splitter` early-returns through the `:501-507`
//!   guard because its outline carries an explicit `boundary` class (quirk #231).
//! * **C** — `applyHoleClearanceOverride` at the real merged 0 µm: fires on all 16 and changes
//!   nothing, which is why the gap went unnoticed for six plans.
//! * **E** / **F** — the non-default hole path at 100 µm and 500 µm, i.e. the µm → board-unit
//!   conversion (board-resolution dependent: 100 µm is 1 000 units on five boards and 10 000 on
//!   `router-rpi-splitter`), the `hole_edge` class, the `Math.max` floor, and the reclassified
//!   circular component keepouts.
//! * **G** / **H** — the two negative early returns.
//! * **I** — a **non-default** copper value (0.0), which the `:501-507` guard cannot stop: it
//!   mutates even `router-rpi-splitter`. This is the arm that shows the guard keys on the
//!   *value*, not on the board.

use std::collections::BTreeMap;

use fr_board::{Board, Item, ItemClass};
use fr_dsn::{BoardReadResult, DsnReadOptions};
use fr_geometry::{Area, Shape};
use fr_router::pipeline::prepare_board;
use fr_settings::RouterSettings;
use fr_settings::sources::DefaultSettings;
use fr_settings::{HostEnvironment, SettingsSource};

const TRANSCRIPT: &str = include_str!("data/p7t15b-clearance-overrides.txt");

/// The stems this test replays in CI. The other seven are heavy enough in a debug build to
/// dominate `cargo test`, so they run under `FR_SLOW_PARITY=1` like the rest of the corpus-wide
/// parity tests (ruling AM).
///
/// The five are chosen for coverage, not for size: `router-rpi-splitter` is the one board the
/// copper override early-returns on *and* the one imperial-resolution board (100 µm = 10 000
/// units), `router-dac2020-bm01` is the 13 000 → 63 000 matrix-sum row from the banked evidence,
/// `router-j2-reference` is the board the 15 254 B / 14 644 B SES measurement was taken on,
/// `drc-dev-board` has both a named non-default clearance class (`Power`) and four circular
/// keepouts, and `batch-empty-board` is the all-zero matrix that becomes a pure 500 µm edge
/// keep-out.
const CI_STEMS: &[&str] = &[
    "router-rpi-splitter",
    "router-dac2020-bm01",
    "router-j2-reference",
    "drc-dev-board",
    "batch-empty-board",
];

// =================================================================================================
// The transcript
// =================================================================================================

/// One `[variant]` block of the transcript: its header values and the whole block's text.
struct Variant {
    name: String,
    copper: Option<f64>,
    hole: Option<f64>,
    block: String,
}

/// One `[board]` block of the transcript.
struct BoardBlock {
    stem: String,
    dsn: String,
    variants: Vec<Variant>,
}

/// `-` is the probe's spelling of Java's `null`.
fn parse_optional(field: &str) -> Option<f64> {
    if field == "-" {
        None
    } else {
        Some(field.parse().unwrap_or_else(|e| panic!("{field}: {e}")))
    }
}

fn field<'a>(line: &'a str, key: &str) -> &'a str {
    line.split_whitespace()
        .find_map(|token| token.strip_prefix(key))
        .unwrap_or_else(|| panic!("no `{key}` in {line:?}"))
}

fn parse_transcript() -> Vec<BoardBlock> {
    let mut boards: Vec<BoardBlock> = Vec::new();
    for line in TRANSCRIPT.lines() {
        if let Some(rest) = line.strip_prefix("[board] ") {
            boards.push(BoardBlock {
                stem: field(rest, "stem=").to_string(),
                dsn: field(rest, "dsn=").to_string(),
                variants: Vec::new(),
            });
        } else if let Some(rest) = line.strip_prefix("[variant] ") {
            let board = boards.last_mut().expect("a [variant] before any [board]");
            board.variants.push(Variant {
                name: field(rest, "name=").to_string(),
                copper: parse_optional(field(rest, "copper=")),
                hole: parse_optional(field(rest, "hole=")),
                block: format!("{line}\n"),
            });
        } else if line.starts_with("  ") {
            let variant = boards
                .last_mut()
                .and_then(|b| b.variants.last_mut())
                .expect("an indented line before any [variant]");
            variant.block.push_str(line);
            variant.block.push('\n');
        }
    }
    boards
}

// =================================================================================================
// The port's side of the same block
// =================================================================================================

/// The headless ladder's priority-0 source, the same one `P6T1.java` and
/// `crates/fr-router/tests/fixtures.rs` use.
fn default_settings() -> RouterSettings {
    DefaultSettings::new(&HostEnvironment::detect())
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone()
}

fn load_board(dsn_rel: &str) -> Board {
    let path = parity::java_dir().join(dsn_rel);
    let file = std::fs::File::open(&path)
        .unwrap_or_else(|e| panic!("cannot open {}: {e}", path.display()));
    let design_name = path
        .file_name()
        .expect("a file name")
        .to_string_lossy()
        .into_owned();
    match fr_dsn::read_board(file, None, Some(&design_name), &DsnReadOptions::default()) {
        BoardReadResult::Success { board, .. } | BoardReadResult::OutlineMissing { board, .. } => {
            *board.unwrap_or_else(|| panic!("{design_name} produced no board"))
        }
        other => panic!("{design_name} did not read: {other:?}"),
    }
}

/// The probe's `[variant]` block, rendered from a port `Board`. Every line is the probe's own
/// format, so the two can be compared as text.
fn render(board: &mut Board, name: &str, copper: Option<f64>, hole: Option<f64>) -> String {
    let mut out = String::new();
    let units = |board: &Board, value: Option<f64>| match value {
        Some(um) => board.clearance_override_board_units(um).to_string(),
        None => "-".to_string(),
    };
    out.push_str(&format!(
        "[variant] name={name} copper={} hole={} copper_units={} hole_units={}\n",
        format_optional(copper),
        format_optional(hole),
        units(board, copper),
        units(board, hole),
    ));

    let layer_count = board.rules.clearance_matrix.get_layer_count();
    let class_count = board.rules.clearance_matrix.get_class_count();
    let names: Vec<String> = (0..class_count)
        .map(|i| {
            board
                .rules
                .clearance_matrix
                .get_name(i)
                .expect("an in-range class")
                .to_string()
        })
        .collect();
    out.push_str(&format!(
        "  layers={layer_count} classes={class_count} names={}\n",
        names.join(",")
    ));

    let default_net_class = board.rules.get_default_net_class();
    let default_area_class_no = board
        .rules
        .net_classes
        .get(default_net_class)
        .default_item_clearance_classes
        .get(ItemClass::Area);
    // The probe prints `-1` for a board with no outline (Java's `null`).
    let outline_class = board
        .get_outline()
        .and_then(|id| board.get_item(id))
        .map(|item| item.clearance_class() as i64)
        .unwrap_or(-1);
    let item_count = board.get_items().count();
    out.push_str(&format!(
        "  outline_class={outline_class} default_area_class={default_area_class_no} \
         hole_clearance={} items={item_count}\n",
        board.rules.get_hole_clearance()
    ));

    // HeadlessBoardManager.java:411-423, transcribed: the exact `ObstacleArea` class, a component
    // id above zero and a circular area. `BTreeMap` is Java's `TreeMap`, i.e. ordered by class
    // index.
    let mut keepout_classes: BTreeMap<usize, usize> = BTreeMap::new();
    let mut keepout_count = 0usize;
    {
        let ctx = board.ctx();
        for item in board.get_items() {
            if let Item::ObstacleArea(keepout) = item
                && keepout.hdr.get_component_id() > 0
                && matches!(keepout.get_area(&ctx), Area::Shape(Shape::Circle(_)))
            {
                keepout_count += 1;
                *keepout_classes
                    .entry(keepout.hdr.clearance_class())
                    .or_insert(0) += 1;
            }
        }
    }
    let histogram: Vec<String> = keepout_classes
        .iter()
        .map(|(class_no, count)| format!("{class_no}:{count}"))
        .collect();
    out.push_str(&format!(
        "  keepouts={keepout_count} keepout_classes={}\n",
        histogram.join(";")
    ));

    let matrix = &board.rules.clearance_matrix;
    let mut matrix_sum: i64 = 0;
    let mut layer_sums: Vec<i64> = vec![0; layer_count];
    let mut row_sums: Vec<Vec<i64>> = vec![vec![0; class_count]; layer_count];
    for (layer, layer_rows) in row_sums.iter_mut().enumerate() {
        for (i, row_sum) in layer_rows.iter_mut().enumerate() {
            for j in 0..class_count {
                let value = i64::from(matrix.get_value(i, j, layer, false));
                matrix_sum += value;
                layer_sums[layer] += value;
                *row_sum += value;
            }
        }
    }
    out.push_str(&format!("  matrix_sum={matrix_sum}\n"));
    for layer in 0..layer_count {
        let rows: Vec<String> = row_sums[layer].iter().map(i64::to_string).collect();
        out.push_str(&format!(
            "  msum L{layer} = {} rows={}\n",
            layer_sums[layer],
            rows.join(",")
        ));
    }
    for (i, name) in names.iter().enumerate() {
        let values: Vec<String> = (0..class_count)
            .map(|j| matrix.get_value(i, j, 0, false).to_string())
            .collect();
        out.push_str(&format!("  mrow L0 {i} {name} = {}\n", values.join(" ")));
    }
    out
}

/// `Double.toString` for the two values the probe prints back — both are whole numbers here, so
/// Java's shortest round-tripping form is `<n>.0`.
fn format_optional(value: Option<f64>) -> String {
    match value {
        None => "-".to_string(),
        Some(v) if v == v.trunc() && v.abs() < 1e7 => format!("{v:.1}"),
        Some(v) => format!("{v}"),
    }
}

// =================================================================================================
// The tests
// =================================================================================================

fn replay(stems: &[&str]) {
    let transcript = parse_transcript();
    assert_eq!(
        transcript.len(),
        16,
        "the committed transcript should carry the whole sixteen-board corpus"
    );
    let mut replayed = 0usize;
    for board_block in &transcript {
        if !stems.contains(&board_block.stem.as_str()) {
            continue;
        }
        let pristine = load_board(&board_block.dsn);
        assert_eq!(
            board_block.variants.len(),
            9,
            "{}: the probe emits nine variants",
            board_block.stem
        );
        for variant in &board_block.variants {
            let mut board = pristine.clone();
            let mut settings = default_settings();
            settings.copper_to_edge_clearance_um = variant.copper;
            settings.hole_clearance_um = variant.hole;
            prepare_board(&mut board, &settings);
            let actual = render(&mut board, &variant.name, variant.copper, variant.hole);
            assert_eq!(
                actual.trim_end(),
                variant.block.trim_end(),
                "{} variant {}: the port's board disagrees with the jar's\n--- port ---\n{}\n--- jar ---\n{}",
                board_block.stem,
                variant.name,
                actual,
                variant.block
            );
        }
        replayed += 1;
    }
    assert_eq!(
        replayed,
        stems.len(),
        "every requested stem must be present in the transcript"
    );
}

#[test]
fn p7t15b_transcript_matches_on_the_ci_stems() {
    if !parity::require_java_dir() {
        return;
    }
    replay(CI_STEMS);
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "slow in debug; run with FR_SLOW_PARITY=1 --release"
)]
fn p7t15b_transcript_matches_on_the_whole_corpus() {
    if !parity::require_java_dir() || std::env::var_os("FR_SLOW_PARITY").is_none() {
        return;
    }
    let stems: Vec<String> = parse_transcript().into_iter().map(|b| b.stem).collect();
    let refs: Vec<&str> = stems.iter().map(String::as_str).collect();
    replay(&refs);
}

/// `fr-board` cannot depend on `fr-settings`, so
/// [`fr_board::DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM`] — which the `:501-507` guard compares
/// against — is a second copy of `DefaultSettings.DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM`. This is
/// the assertion that keeps the two from drifting apart.
#[test]
fn the_two_copies_of_the_default_copper_to_edge_clearance_agree() {
    assert_eq!(
        fr_board::DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM,
        DefaultSettings::DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM
    );
}

/// The headless ladder's priority-0 source fills both knobs, so a real CLI run reaches both
/// overrides — `DefaultSettings.java:78, :81`. Without this, `prepare_board` would be a
/// no-op-by-omission rather than by design.
#[test]
fn the_default_settings_ladder_fills_both_override_knobs() {
    let settings: RouterSettings = default_settings();
    assert_eq!(settings.copper_to_edge_clearance_um, Some(500.0));
    assert_eq!(settings.hole_clearance_um, Some(0.0));
}

/// `prepare_board` skips an override whose setting is `None`, which is Java's
/// `routerSettings.copperToEdgeClearanceUm == null` guard (HeadlessBoardManager.java:470-471,
/// :350-351) — and it applies copper **before** hole, which is what makes `board_edge` take the
/// lower class index when both fire.
#[test]
fn prepare_board_skips_a_none_setting_and_orders_copper_before_hole() {
    if !parity::require_java_dir() {
        return;
    }
    let pristine = load_board("fixtures/Issue575-drc_dev-board_4_hole_clearance_violations.dsn");

    let mut none_at_all = pristine.clone();
    let mut settings = default_settings();
    settings.copper_to_edge_clearance_um = None;
    settings.hole_clearance_um = None;
    assert!(!prepare_board(&mut none_at_all, &settings));
    assert_eq!(
        none_at_all.rules.clearance_matrix.get_class_count(),
        pristine.rules.clearance_matrix.get_class_count()
    );

    let mut both = pristine.clone();
    settings.copper_to_edge_clearance_um = Some(500.0);
    settings.hole_clearance_um = Some(500.0);
    assert!(prepare_board(&mut both, &settings));
    let board_edge = both
        .rules
        .clearance_matrix
        .get_no(fr_board::BOARD_EDGE_CLEARANCE_CLASS_NAME)
        .expect("the copper override appended board_edge");
    let hole_edge = both
        .rules
        .clearance_matrix
        .get_no(fr_board::HOLE_EDGE_CLEARANCE_CLASS_NAME)
        .expect("the hole override appended hole_edge");
    assert!(
        board_edge < hole_edge,
        "copper runs first, so board_edge takes the lower index ({board_edge} vs {hole_edge})"
    );
}
