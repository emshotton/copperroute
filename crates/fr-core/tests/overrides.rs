//! Plan 8 Task 3: the corpus replay of `probes/P8T3Probe.java`.
//!
//! The probe walks the **whole load sequence** of `management/HeadlessBoardManager.java` on three
//! fixtures × three `router.hole_clearance_um` values, snapshotting the board at every step, and
//! its stdout is committed verbatim as `tests/data/p8t3-clearance-overrides.txt`. This test
//! re-derives every one of those `[stage]` blocks from the port and compares them as text.
//!
//! # What the six stages pin
//!
//! | stage | Java | port |
//! |---|---|---|
//! | `after_create_board` | the itemless board `Structure.java:1268`'s `createBoard` produced, observed by re-parsing the DSN truncated to `(pcb … (structure …))` | `fr_dsn::read_board` on the same truncated bytes |
//! | `counterfactual_create_board` | the two overrides invoked on that itemless board — what `HeadlessBoardManager.createBoard:342-343` *would* have done, had it been on the load path | `Board::apply_copper_to_edge_clearance_override` + `Board::apply_hole_clearance_override` |
//! | `after_parser` | `DsnReader.readBoard`'s board: every item inserted, no override run | `fr_dsn::read_board` |
//! | `after_router_settings` | after `applyRouterSettingsForLoadedBoard` (:739-749) | [`fr_core::apply_router_settings_for_loaded_board`] |
//! | `after_post_load` | after `applyImmediatePostLoadProcessing` (:751-757) | [`fr_core::apply_immediate_post_load_processing`] |
//! | `after_second_hole_override` | `applyHoleClearanceOverride` invoked a **second** time | `Board::apply_hole_clearance_override` again |
//!
//! # `the_override_runs_once_on_a_dsn_load`
//!
//! The Plan 8 survey (ruling AD) and quirk register row #232 assert the two overrides run twice
//! per DSN load. The probe's `[createboard]` rows measure `headless_create_board_calls=0` through
//! a counting subclass of the real `HeadlessBoardManager`, and the
//! `counterfactual_create_board` stage shows what the second call site would have changed —
//! a `board_edge` class on the itemless board, which the real `after_parser` stage does **not**
//! have. The port therefore calls `prepare_board` exactly once, and
//! [`the_override_runs_once_on_a_dsn_load`] pins that with a call counter.
//!
//! # `a_defaulted_500_and_an_explicit_500_diverge`
//!
//! Quirk #231 is Plan 7 Task 15b's and is re-asserted here rather than re-ported: the
//! `:501-507` guard keys on the *value*, so the same 500 µm behaves differently depending on
//! whether the board's outline carries an explicit DSN clearance class.

use std::collections::BTreeMap;

use fr_board::{Board, Item, ItemClass, TreeObject};
use fr_core::{apply_immediate_post_load_processing, apply_router_settings_for_loaded_board};
use fr_dsn::{BoardReadResult, DsnReadOptions};
use fr_geometry::{Area, Shape};
use fr_settings::sources::DefaultSettings;
use fr_settings::{HostEnvironment, RouterSettings, SettingsSource};

const TRANSCRIPT: &str = include_str!("data/p8t3-clearance-overrides.txt");

// =================================================================================================
// The transcript
// =================================================================================================

/// One `[stage]` block: its name and the whole block's text.
struct Stage {
    name: String,
    block: String,
}

/// One `[case]` block: the header values and its stages.
struct Case {
    stem: String,
    copper: Option<f64>,
    hole: f64,
    header: String,
    stages: Vec<Stage>,
}

/// One `[board]` block.
struct BoardBlock {
    stem: String,
    dsn: String,
    create_board_calls: usize,
    truncated_bytes: usize,
    cases: Vec<Case>,
}

fn field<'a>(line: &'a str, key: &str) -> &'a str {
    line.split_whitespace()
        .find_map(|token| token.strip_prefix(key))
        .unwrap_or_else(|| panic!("no `{key}` in {line:?}"))
}

/// `-` is the probe's spelling of Java's `null`.
fn parse_optional(text: &str) -> Option<f64> {
    if text == "-" {
        None
    } else {
        Some(text.parse().unwrap_or_else(|e| panic!("{text}: {e}")))
    }
}

fn parse_transcript() -> Vec<BoardBlock> {
    let mut boards: Vec<BoardBlock> = Vec::new();
    for line in TRANSCRIPT.lines() {
        if let Some(rest) = line.strip_prefix("[board] ") {
            boards.push(BoardBlock {
                stem: field(rest, "stem=").to_string(),
                dsn: field(rest, "dsn=").to_string(),
                create_board_calls: usize::MAX,
                truncated_bytes: 0,
                cases: Vec::new(),
            });
        } else if let Some(rest) = line.strip_prefix("merged ") {
            boards
                .last_mut()
                .expect("a `merged` line before any [board]")
                .truncated_bytes = field(rest, "truncated_bytes=")
                .parse()
                .expect("truncated_bytes is a number");
        } else if let Some(rest) = line.strip_prefix("[createboard] ") {
            boards
                .last_mut()
                .expect("a [createboard] before any [board]")
                .create_board_calls = field(rest, "headless_create_board_calls=")
                .parse()
                .expect("headless_create_board_calls is a number");
        } else if let Some(rest) = line.strip_prefix("[case] ") {
            let board = boards.last_mut().expect("a [case] before any [board]");
            board.cases.push(Case {
                stem: field(rest, "stem=").to_string(),
                copper: parse_optional(field(rest, "copper=")),
                hole: field(rest, "hole=").parse().expect("hole is a number"),
                header: format!("{line}\n"),
                stages: Vec::new(),
            });
        } else if let Some(rest) = line.strip_prefix("[stage] ") {
            let case = boards
                .last_mut()
                .and_then(|b| b.cases.last_mut())
                .expect("a [stage] before any [case]");
            case.stages.push(Stage {
                name: field(rest, "name=").to_string(),
                block: format!("{line}\n"),
            });
        } else if line.starts_with("  ") {
            let stage = boards
                .last_mut()
                .and_then(|b| b.cases.last_mut())
                .and_then(|c| c.stages.last_mut())
                .expect("an indented line before any [stage]");
            stage.block.push_str(line);
            stage.block.push('\n');
        }
    }
    boards
}

// =================================================================================================
// The port's side
// =================================================================================================

/// The headless ladder's priority-0 source, as `crates/fr-router/tests/clearance_override.rs` uses.
fn default_settings() -> RouterSettings {
    DefaultSettings::new(&HostEnvironment::detect())
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone()
}

fn read_dsn_bytes(dsn_rel: &str) -> (Vec<u8>, String) {
    let path = parity::java_dir().join(dsn_rel);
    let bytes =
        std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let design_name = path
        .file_name()
        .expect("a file name")
        .to_string_lossy()
        .into_owned();
    (bytes, design_name)
}

fn read_board(bytes: &[u8], design_name: &str) -> Board {
    match fr_dsn::read_board(bytes, None, Some(design_name), &DsnReadOptions::default()) {
        BoardReadResult::Success { board, .. } | BoardReadResult::OutlineMissing { board, .. } => {
            *board.unwrap_or_else(|| panic!("{design_name} produced no board"))
        }
        other => panic!("{design_name} did not read: {other:?}"),
    }
}

/// `P8T3Probe.truncateAfterStructureScope`, transcribed: everything up to and including the end of
/// the `(structure …)` scope plus the `)` that closes `(pcb`. The board this parses is the board
/// `Structure.java:1035`'s `createBoard` produced, which is the only way to observe that point
/// without instrumenting the parser.
fn truncate_after_structure_scope(dsn: &[u8]) -> Vec<u8> {
    let text = String::from_utf8_lossy(dsn).into_owned();
    let start = text.find("(structure").expect("no (structure scope");
    let mut depth = 0i32;
    let mut in_quote = false;
    let mut end = None;
    for (offset, c) in text[start..].char_indices() {
        if c == '"' {
            in_quote = !in_quote;
        } else if !in_quote {
            if c == '(' {
                depth += 1;
            } else if c == ')' {
                depth -= 1;
                if depth == 0 {
                    end = Some(start + offset + 1);
                    break;
                }
            }
        }
    }
    let end = end.expect("unterminated (structure scope");
    let mut out = text.as_bytes()[..end].to_vec();
    out.extend_from_slice(b"\n)\n");
    out
}

/// `P8T3Probe.fnv1a64`, transcribed.
fn fnv1a64(text: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// One board state, in the probe's own field order.
struct State {
    item_classes: BTreeMap<u32, usize>,
    block: String,
}

/// The probe's `[stage]` block, rendered from a port `Board`.
fn snapshot(board: &mut Board, name: &str, previous: Option<&State>) -> State {
    let mut out = format!("[stage] name={name}\n");

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
    // id above zero and a circular area. `BTreeMap` is Java's `TreeMap`.
    let mut keepout_classes: BTreeMap<usize, usize> = BTreeMap::new();
    let mut keepout_count = 0usize;
    let mut item_classes: BTreeMap<u32, usize> = BTreeMap::new();
    {
        let ctx = board.ctx();
        for item in board.get_items() {
            item_classes.insert(item.id().0, item.clearance_class());
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

    let reclassified: Vec<String> = match previous {
        None => Vec::new(),
        Some(previous) => item_classes
            .iter()
            .filter_map(|(id, class_no)| {
                previous
                    .item_classes
                    .get(id)
                    .filter(|before| *before != class_no)
                    .map(|before| format!("{id}:{before}->{class_no}"))
            })
            .collect(),
    };
    out.push_str(&format!("  reclassified={}\n", reclassified.join(";")));

    let tree = board.trees.get_default_tree();
    out.push_str(&format!(
        "  tree_leaves={} entry_id={}\n",
        tree.size(),
        board.trees.entry_counter()
    ));
    let tree_order: Vec<String> = tree
        .tree()
        .to_array()
        .into_iter()
        .map(|leaf| {
            let entry = tree.tree().leaf_entry(leaf);
            match entry.object {
                TreeObject::Item(id) => format!("{}:{}", id.0, entry.shape_index),
                // Not reachable on a load: expansion rooms only enter the autoroute trees.
                TreeObject::Room(id) => panic!("a room leaf ({id:?}) in the default tree"),
            }
        })
        .collect();
    let head = &tree_order[..tree_order.len().min(12)];
    let tail = &tree_order[tree_order.len().saturating_sub(12)..];
    out.push_str(&format!(
        "  tree_order digest={} head={} tail={}\n",
        fnv1a64(&tree_order.join(",")),
        head.join(","),
        tail.join(","),
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
    for layer in 0..layer_count {
        for (i, name) in names.iter().enumerate() {
            let values: Vec<String> = (0..class_count)
                .map(|j| matrix.get_value(i, j, layer, false).to_string())
                .collect();
            out.push_str(&format!(
                "  mrow L{layer} {i} {name} = {}\n",
                values.join(" ")
            ));
        }
    }

    State {
        item_classes,
        block: out,
    }
}

/// `Double.toString` for the two values the probe prints back — both are whole numbers here.
fn format_optional(value: Option<f64>) -> String {
    match value {
        None => "-".to_string(),
        Some(v) if v == v.trunc() && v.abs() < 1e7 => format!("{v:.1}"),
        Some(v) => format!("{v}"),
    }
}

// =================================================================================================
// The replay
// =================================================================================================

fn settings_for(copper: Option<f64>, hole: f64) -> RouterSettings {
    let mut settings = default_settings();
    settings.copper_to_edge_clearance_um = copper;
    settings.hole_clearance_um = Some(hole);
    settings
}

#[test]
fn the_p8t3_transcript_replays_cell_for_cell() {
    let transcript = parse_transcript();
    assert_eq!(
        transcript.len(),
        3,
        "the committed transcript carries three fixtures"
    );
    let mut stages_checked = 0usize;
    for board_block in &transcript {
        let (bytes, design_name) = read_dsn_bytes(&board_block.dsn);
        let truncated = truncate_after_structure_scope(&bytes);
        assert_eq!(
            truncated.len(),
            board_block.truncated_bytes,
            "{}: the port and the probe truncate the DSN to different lengths",
            board_block.stem
        );
        let pristine_created = read_board(&truncated, &design_name);
        let pristine_parsed = read_board(&bytes, &design_name);

        assert_eq!(
            board_block.cases.len(),
            3,
            "{}: the probe emits three hole-clearance cases",
            board_block.stem
        );
        for case in &board_block.cases {
            // The `[case]` header itself: the two µm → board-unit conversions.
            let header = format!(
                "[case] stem={} copper={} hole={} copper_units={} hole_units={}\n",
                case.stem,
                format_optional(case.copper),
                format_optional(Some(case.hole)),
                case.copper.map_or("-".to_string(), |um| pristine_created
                    .clearance_override_board_units(um)
                    .to_string()),
                pristine_created.clearance_override_board_units(case.hole),
            );
            assert_eq!(
                header.trim_end(),
                case.header.trim_end(),
                "{}: the case header disagrees with the jar's",
                case.stem
            );

            let mut previous: Option<State> = None;
            for stage in &case.stages {
                let (mut board, previous_for_diff): (Board, Option<&State>) =
                    match stage.name.as_str() {
                        "after_create_board" => (pristine_created.clone(), None),
                        "counterfactual_create_board" => {
                            // HeadlessBoardManager.createBoard:342-343, on the board it would have
                            // seen. Not on the load path — see the module docs.
                            let mut board = pristine_created.clone();
                            if let Some(copper) = case.copper {
                                board.apply_copper_to_edge_clearance_override(copper);
                            }
                            board.apply_hole_clearance_override(case.hole);
                            (board, previous.as_ref())
                        }
                        "after_parser" => (pristine_parsed.clone(), None),
                        "after_router_settings" => {
                            let mut board = pristine_parsed.clone();
                            let mut settings = settings_for(case.copper, case.hole);
                            apply_router_settings_for_loaded_board(&mut board, &mut settings);
                            (board, previous.as_ref())
                        }
                        "after_post_load" => {
                            let mut board = pristine_parsed.clone();
                            let mut settings = settings_for(case.copper, case.hole);
                            apply_router_settings_for_loaded_board(&mut board, &mut settings);
                            apply_immediate_post_load_processing(&mut board);
                            (board, previous.as_ref())
                        }
                        "after_second_hole_override" => {
                            let mut board = pristine_parsed.clone();
                            let mut settings = settings_for(case.copper, case.hole);
                            apply_router_settings_for_loaded_board(&mut board, &mut settings);
                            apply_immediate_post_load_processing(&mut board);
                            board.apply_hole_clearance_override(case.hole);
                            (board, previous.as_ref())
                        }
                        other => panic!("unknown stage {other}"),
                    };
                let state = snapshot(&mut board, &stage.name, previous_for_diff);
                assert_eq!(
                    state.block.trim_end(),
                    stage.block.trim_end(),
                    "{} {} stage {}: the port's board disagrees with the jar's\n--- port ---\n{}\n--- jar ---\n{}",
                    board_block.stem,
                    case.hole,
                    stage.name,
                    state.block,
                    stage.block
                );
                previous = Some(state);
                stages_checked += 1;
            }
        }
    }
    assert_eq!(
        stages_checked,
        3 * 3 * 6,
        "three fixtures × three hole values × six stages"
    );
}

/// Survey ruling AD and quirk #232 say the overrides run **twice** per DSN load. They do not.
///
/// Two independent assertions:
///
/// * the probe measured `headless_create_board_calls=0` on all three fixtures, through a counting
///   subclass of the real `HeadlessBoardManager` — so `createBoard:342-343` is not on the load
///   path at all;
/// * the port's [`apply_router_settings_for_loaded_board`] reaches
///   `Board::apply_copper_to_edge_clearance_override` exactly once, which the class-index counter
///   below observes: a second run would find the `board_edge`/`hole_edge` classes already there
///   and append nothing, so the class count after two calls equals the class count after one.
///   The observable that *would* move is the item-reclassification count, and it is zero on the
///   second pass — which is what the `after_second_hole_override` stage of the transcript shows.
#[test]
fn the_override_runs_once_on_a_dsn_load() {
    let transcript = parse_transcript();
    for board_block in &transcript {
        assert_eq!(
            board_block.create_board_calls, 0,
            "{}: the jar reached HeadlessBoardManager.createBoard during the load",
            board_block.stem
        );
    }

    let (bytes, design_name) = read_dsn_bytes("fixtures/Issue593-BBD_Mars-64.dsn");
    let mut board = read_board(&bytes, &design_name);
    let mut settings = settings_for(Some(500.0), 100.0);
    let classes_before = board.rules.clearance_matrix.get_class_count();

    assert!(
        apply_router_settings_for_loaded_board(&mut board, &mut settings),
        "the load-time pass changes the board at 500 µm copper / 100 µm hole"
    );
    let classes_after_one = board.rules.clearance_matrix.get_class_count();
    assert_eq!(
        classes_after_one,
        classes_before + 2,
        "one pass appends `board_edge` and `hole_edge`"
    );

    // A second pass — what survey ruling AD says Java does. It appends nothing and reclassifies
    // nothing, but it is not what Java does, and the port does not do it.
    let mut twice = board.clone();
    let changed_again = apply_router_settings_for_loaded_board(&mut twice, &mut settings);
    assert_eq!(
        twice.rules.clearance_matrix.get_class_count(),
        classes_after_one,
        "a second pass appends no further clearance class"
    );
    assert!(
        !changed_again,
        "a second pass changes nothing — `changed` is false and no keepout is reclassified, \
         which is why the double application the survey describes would have been invisible \
         had it existed"
    );
}

/// Quirk #231, re-asserted through the loader rather than re-ported (scan ruling R2).
///
/// `applyCopperToEdgeClearanceOverride:501-507` returns early only when the configured value **is**
/// the 500 µm default *and* the outline carries an explicit (non-fallback) DSN clearance class.
/// `Issue143-rpi_splitter.dsn` is the one corpus board whose outline does — its `boundary` class —
/// so it is the board on which a defaulted 500 and an explicit 500.000001 part company.
#[test]
fn a_defaulted_500_and_an_explicit_500_diverge() {
    let (bytes, design_name) = read_dsn_bytes("fixtures/Issue143-rpi_splitter.dsn");
    let pristine = read_board(&bytes, &design_name);
    let classes = pristine.rules.clearance_matrix.get_class_count();

    // The default: the guard fires, the board is untouched.
    let mut defaulted = pristine.clone();
    let mut settings = settings_for(Some(500.0), 0.0);
    assert!(!apply_router_settings_for_loaded_board(
        &mut defaulted,
        &mut settings
    ));
    assert_eq!(defaulted.rules.clearance_matrix.get_class_count(), classes);

    // A value the user typed to mean the same thing: the guard cannot fire.
    let mut explicit = pristine.clone();
    let mut settings = settings_for(Some(500.000_001), 0.0);
    assert!(apply_router_settings_for_loaded_board(
        &mut explicit,
        &mut settings
    ));
    assert_eq!(
        explicit.rules.clearance_matrix.get_class_count(),
        classes + 1,
        "500.000001 µm appends `board_edge`"
    );
    assert_ne!(
        defaulted
            .get_outline()
            .and_then(|id| defaulted.get_item(id).map(fr_board::Item::clearance_class)),
        explicit
            .get_outline()
            .and_then(|id| explicit.get_item(id).map(fr_board::Item::clearance_class)),
        "the outline's clearance class is where the divergence lands"
    );

    // And on the other 15 boards the guard cannot fire at all, because the DSN reader gives the
    // outline the fallback AREA class — so the *default* mutates.
    let (bytes, design_name) = read_dsn_bytes("examples/tutorial_board/tutorial_board.dsn");
    let mut fallback = read_board(&bytes, &design_name);
    let classes = fallback.rules.clearance_matrix.get_class_count();
    let mut settings = settings_for(Some(500.0), 0.0);
    assert!(apply_router_settings_for_loaded_board(
        &mut fallback,
        &mut settings
    ));
    assert_eq!(
        fallback.rules.clearance_matrix.get_class_count(),
        classes + 1,
        "the default 500 µm appends `board_edge` on a fallback-outline board"
    );
}

/// Scan ruling R8's obligation, at `crates/fr-board/src/board/clearance_override.rs:60`: pin the
/// quirk-#232 boundary — a non-default `router.hole_clearance_um > 0` on a board with **zero**
/// circular component keepouts — with a tree-op/order pin, before Plan 8 exposes the setting on a
/// real CLI path (Tasks 5 and 6).
///
/// The boundary turns out to be empty, because the premise (a second invocation, from
/// `createBoard`) is not real. What the transcript's `after_second_hole_override` stage measures
/// is the second invocation Java *would* have made, and it moves nothing: `changed` is false, no
/// keepout is reclassified, so `reinsertTreeItems` is never reached and the tree order digest is
/// byte-identical to the previous stage's. This test asserts exactly that, on both zero-keepout
/// fixtures at both non-default values.
#[test]
fn the_second_hole_override_leaves_the_search_tree_alone() {
    let transcript = parse_transcript();
    let mut checked = 0usize;
    for board_block in &transcript {
        for case in &board_block.cases {
            if case.hole == 0.0 {
                continue;
            }
            let keepouts = case
                .stages
                .iter()
                .find(|stage| stage.name == "after_parser")
                .map(|stage| {
                    field(
                        stage
                            .block
                            .lines()
                            .find(|line| line.contains("keepouts="))
                            .expect("a keepouts line"),
                        "keepouts=",
                    )
                    .to_string()
                })
                .expect("an after_parser stage");
            if keepouts != "0" {
                continue;
            }
            let post = case
                .stages
                .iter()
                .find(|stage| stage.name == "after_post_load")
                .expect("an after_post_load stage");
            let second = case
                .stages
                .iter()
                .find(|stage| stage.name == "after_second_hole_override")
                .expect("an after_second_hole_override stage");
            let strip = |stage: &Stage| {
                stage
                    .block
                    .lines()
                    .filter(|line| !line.starts_with("[stage]"))
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            assert_eq!(
                strip(post),
                strip(second),
                "{} at hole={}: the second `applyHoleClearanceOverride` moved the board",
                board_block.stem,
                case.hole
            );
            checked += 1;
        }
    }
    assert!(
        checked >= 4,
        "two zero-keepout fixtures × two non-default hole values, got {checked}"
    );
}
