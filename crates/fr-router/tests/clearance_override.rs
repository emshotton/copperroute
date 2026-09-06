use std::collections::BTreeMap;

use fr_board::{Board, Item, ItemClass};
use fr_dsn::{BoardReadResult, DsnReadOptions};
use fr_geometry::{Area, Shape};
use fr_router::pipeline::prepare_board;
use fr_settings::RouterSettings;
use fr_settings::sources::DefaultSettings;
use fr_settings::{HostEnvironment, SettingsSource};

const TRANSCRIPT: &str = include_str!("data/p7t15b-clearance-overrides.txt");

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

fn default_settings() -> RouterSettings {
    DefaultSettings::new(&HostEnvironment::detect())
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone()
}

fn load_board(dsn_rel: &str) -> Board {
    let path = parity::reference_dir().join(dsn_rel);
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

/// The `(stem, variant)` pairs on which the port deliberately disagrees with the committed jar
/// transcript, each with its register row and its reason.
///
/// Both directions are checked by [`replay`]: a pair that differs without an entry fails, and an
/// entry whose pair no longer differs fails.
const KNOWN_DIVERGENCES: &[(&str, &str, &str, &str)] = &[
    (
        "router-rpi-splitter",
        "B",
        "#231",
        "the one corpus board whose outline (`boundary`) carries an explicit, non-fallback DSN \
         clearance class, and so the only one Java's `:501-507` guard could stop. The jar \
         early-returns at the default 500 µm and leaves the board pristine; the port applies it, \
         appending `board_edge`, writing 500 µm into its row and column on every layer and \
         re-pointing the outline — the same thing it does on the other fifteen. Quirk #231's fix \
         made the option continuous, so this board is no longer the exception.",
    ),
    (
        "router-rpi-splitter",
        "D",
        "#231",
        "B's divergence with the 0 µm hole override beside it; the hole half is identical on both \
         sides and contributes nothing to the diff.",
    ),
];

fn replay(stems: &[&str]) {
    let transcript = parse_transcript();
    assert_eq!(
        transcript.len(),
        16,
        "the committed transcript should carry the whole sixteen-board corpus"
    );
    let mut replayed = 0usize;
    let mut diverged: Vec<(String, String)> = Vec::new();
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
            let known = KNOWN_DIVERGENCES
                .iter()
                .find(|(stem, name, _, _)| *stem == board_block.stem && *name == variant.name);
            if actual.trim_end() == variant.block.trim_end() {
                if let Some((stem, name, row, reason)) = known {
                    panic!(
                        "`{stem}` variant {name} now MATCHES the jar — delete its \
                         KNOWN_DIVERGENCES entry ({row}: {reason})"
                    );
                }
                continue;
            }
            assert!(
                known.is_some(),
                "{} variant {}: the port's board disagrees with the jar's and no \
                 KNOWN_DIVERGENCES entry names it — add one with the register row that \
                 authorises it\n--- port ---\n{}\n--- jar ---\n{}",
                board_block.stem,
                variant.name,
                actual,
                variant.block
            );
            diverged.push((board_block.stem.clone(), variant.name.clone()));
        }
        replayed += 1;
    }
    assert_eq!(
        replayed,
        stems.len(),
        "every requested stem must be present in the transcript"
    );
    // The other direction, for the stems this call actually replayed: every entry whose stem was
    // in scope must have been exercised.
    for (stem, name, row, reason) in KNOWN_DIVERGENCES {
        if !stems.contains(stem) {
            continue;
        }
        assert!(
            diverged.iter().any(|(s, n)| s == stem && n == name),
            "`{stem}` variant {name} was replayed and did not diverge — delete its \
             KNOWN_DIVERGENCES entry ({row}: {reason})"
        );
    }
}

#[test]
fn p7t15b_transcript_matches_on_the_ci_stems() {
    if !parity::require_reference_dir() {
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
    if !parity::require_reference_dir() || std::env::var_os("FR_SLOW_PARITY").is_none() {
        return;
    }
    let stems: Vec<String> = parse_transcript().into_iter().map(|b| b.stem).collect();
    let refs: Vec<&str> = stems.iter().map(String::as_str).collect();
    replay(&refs);
}

/// `fr-board` cannot depend on `fr-settings`, so
/// [`fr_board::DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM`] is a second copy of
/// `DefaultSettings.DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM`. Quirk #231's fix removed the
/// `:501-507` comparison that used to read it, but the constant is still the default a CLI run
/// carries; this is the assertion that keeps the two from drifting apart.
#[test]
fn the_two_copies_of_the_default_copper_to_edge_clearance_agree() {
    assert_eq!(
        fr_board::DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM,
        DefaultSettings::DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM
    );
}

#[test]
fn the_default_settings_ladder_fills_both_override_knobs() {
    let settings: RouterSettings = default_settings();
    assert_eq!(settings.copper_to_edge_clearance_um, Some(500.0));
    assert_eq!(settings.hole_clearance_um, Some(0.0));
}

#[test]
fn prepare_board_skips_a_none_setting_and_orders_copper_before_hole() {
    if !parity::require_reference_dir() {
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
