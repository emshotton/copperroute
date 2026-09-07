use copper_board::prelude::*;
use copper_dsn::BoardReadResult;
use copper_dsn::parser::scope_parameter::DsnReadOptions;
use copper_geometry::{IntBox, PolylineShapeRef, TileShape};
use copper_settings::{LayerSettings, RouterSettings};

fn synthetic_board(width: i32, height: i32, is_signal: &[bool]) -> Board {
    let layers = LayerStructure::new(
        is_signal
            .iter()
            .enumerate()
            .map(|(i, signal)| Layer::new(format!("L{i}"), *signal))
            .collect(),
    );
    let clearance_matrix = ClearanceMatrix::get_default_instance(&layers, 10);
    let mut rules = BoardRules::new(layers.clone(), clearance_matrix);
    rules.create_default_net_class();
    let box_ = IntBox::from_coords(0, 0, width, height);
    Board::new(
        vec![PolylineShapeRef::Tile(TileShape::Box(box_))],
        0,
        box_,
        rules,
        BoardLibrary::new(Padstacks::new(layers), Packages::new()),
        Components::new(),
        Communication::default(),
    )
}

fn fixture_board(name: &str) -> Board {
    let path = testkit::fixture(name);
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("cannot read fixture {}: {e}", path.display()));
    match copper_dsn::read_board(&bytes[..], None, Some(name), &DsnReadOptions::default()) {
        BoardReadResult::Success { board, .. } | BoardReadResult::OutlineMissing { board, .. } => {
            *board.unwrap_or_else(|| panic!("{name} produced no board"))
        }
        other => panic!("{name} did not read: {other:?}"),
    }
}

fn rows(settings: &RouterSettings) -> Vec<(bool, bool, f64, f64, f64)> {
    (0..settings.get_layer_count())
        .map(|i| {
            (
                settings.get_layer_active(i),
                settings.get_preferred_direction_is_horizontal(i),
                settings.get_bend_cost(i),
                settings.get_preferred_direction_trace_costs(i),
                settings.get_against_preferred_direction_trace_costs(i),
            )
        })
        .collect()
}

fn sized_settings(layer_count: usize) -> RouterSettings {
    let mut settings = RouterSettings::new();
    settings.set_layer_count(layer_count);
    settings
}

#[test]
fn apply_board_specific_optimizations_initializes_trace_costs_once() {
    let board = synthetic_board(2_000_000, 1_000_000, &[true, true]);
    let mut settings = sized_settings(2);
    assert!(
        !settings.are_board_specific_trace_costs_applied(),
        "setLayerCount's reallocation branch clears the flag (RouterSettings.java:457)"
    );

    settings.apply_board_specific_optimizations(&board);

    assert!(settings.are_board_specific_trace_costs_applied());
    assert!(settings.get_preferred_direction_is_horizontal(0));
    assert!(!settings.get_preferred_direction_is_horizontal(1));
    assert_eq!(settings.get_preferred_direction_trace_costs(0), 1.0);
    assert_eq!(settings.get_preferred_direction_trace_costs(1), 1.0);
    assert_eq!(settings.get_against_preferred_direction_trace_costs(0), 3.0);
    assert_eq!(settings.get_against_preferred_direction_trace_costs(1), 1.5);
    assert_eq!(
        rows(&settings),
        vec![(true, true, 0.0, 1.0, 3.0), (true, false, 0.0, 1.0, 1.5),]
    );

    let before = rows(&settings);
    settings.apply_board_specific_optimizations(&board);
    assert_eq!(
        rows(&settings),
        before,
        "the guarded second call is a no-op"
    );
}

#[test]
fn apply_board_specific_optimizations_preserves_user_trace_costs_on_second_call() {
    let board = synthetic_board(2_000_000, 1_000_000, &[true, true]);
    let mut settings = sized_settings(2);
    settings.apply_board_specific_optimizations(&board);

    settings.set_against_preferred_direction_trace_costs(0, 4.5);
    settings.set_against_preferred_direction_trace_costs(1, 3.2);
    settings.set_preferred_direction_trace_costs(0, 2.0);

    settings.apply_board_specific_optimizations(&board);

    assert_eq!(settings.get_against_preferred_direction_trace_costs(0), 4.5);
    assert_eq!(settings.get_against_preferred_direction_trace_costs(1), 3.2);
    assert_eq!(settings.get_preferred_direction_trace_costs(0), 2.0);
    assert_eq!(settings.get_preferred_direction_trace_costs(1), 1.0);
}

#[test]
fn apply_board_specific_optimizations_if_needed_skips_when_already_board_tuned() {
    let board = synthetic_board(2_000_000, 1_000_000, &[true, true]);
    let mut settings = sized_settings(2);
    settings.apply_board_specific_optimizations(&board);
    settings.set_against_preferred_direction_trace_costs(0, 5.0);
    settings.set_against_preferred_direction_trace_costs(1, 6.0);

    settings.apply_board_specific_optimizations_if_needed(&board);

    assert_eq!(settings.get_against_preferred_direction_trace_costs(0), 5.0);
    assert_eq!(settings.get_against_preferred_direction_trace_costs(1), 6.0);
}

#[test]
fn apply_board_specific_optimizations_if_needed_runs_when_layer_count_mismatch() {
    let board = synthetic_board(2_000_000, 1_000_000, &[true, true]);
    let mut untuned = RouterSettings::new();
    assert_eq!(untuned.get_layer_count(), 0);
    assert!(!untuned.are_board_specific_trace_costs_applied());

    untuned.apply_board_specific_optimizations_if_needed(&board);

    assert_eq!(untuned.get_layer_count(), 2);
    assert!(untuned.are_board_specific_trace_costs_applied());
    assert!(untuned.get_layer_active(0));
    assert!(untuned.get_layer_active(1));
    assert_eq!(
        rows(&untuned),
        vec![(true, true, 0.0, 1.0, 3.0), (true, false, 0.0, 1.0, 1.5),]
    );
}

#[test]
fn apply_board_specific_optimizations_preserves_settings() {
    let board = synthetic_board(2_000_000, 2_000_000, &[true, true]);
    let mut settings = sized_settings(2);
    let layers = settings
        .layers
        .as_mut()
        .expect("setLayerCount allocated it");
    layers[0].routable = Some(false);
    layers[1].routable = Some(true);
    layers[0].preferred_direction_horizontal = Some(true);
    layers[1].preferred_direction_horizontal = Some(false);

    settings.apply_board_specific_optimizations(&board);

    let layers = settings.layers.as_ref().expect("still there");
    assert_eq!(layers[0].routable, Some(false));
    assert_eq!(layers[1].routable, Some(true));
    assert_eq!(layers[0].preferred_direction_horizontal, Some(true));
    assert_eq!(layers[1].preferred_direction_horizontal, Some(false));

    assert_eq!(settings.get_against_preferred_direction_trace_costs(0), 2.0);
    assert_eq!(settings.get_against_preferred_direction_trace_costs(1), 2.0);
}

#[test]
fn outer_layer_bonus_on_a_four_signal_layer_board() {
    let board = synthetic_board(2_000_000, 1_000_000, &[true, true, true, true]);
    let mut settings = sized_settings(4);

    settings.apply_board_specific_optimizations(&board);

    assert_eq!(
        rows(&settings),
        vec![
            (true, true, 0.0, 1.8, 3.8),
            (true, false, 0.0, 1.0, 1.5),
            (true, true, 0.0, 1.0, 3.0),
            (true, false, 0.0, 1.8, 2.3),
        ]
    );
}

#[test]
fn non_signal_layers_are_forced_unroutable_and_do_not_toggle_the_direction() {
    let board = synthetic_board(2_000_000, 1_000_000, &[true, false, true]);
    let mut settings = sized_settings(3);

    settings.apply_board_specific_optimizations(&board);

    let layers = settings.layers.as_ref().expect("allocated");
    assert_eq!(layers[1].routable, Some(false));
    assert_eq!(
        rows(&settings),
        vec![
            (true, true, 0.0, 1.0, 3.0),
            (false, true, 0.0, 1.0, 3.0),
            (true, false, 0.0, 1.0, 1.5),
        ],
        "signalLayerCount() == 2, so no outer-layer bonus either"
    );
}

#[test]
fn absent_bend_costs_take_the_default_unclamped() {
    let board = synthetic_board(2_000_000, 1_000_000, &[true, true]);
    let mut settings = sized_settings(2);
    settings
        .scoring
        .as_mut()
        .expect("new() allocates scoring")
        .default_bend_cost = Some(99.0);
    settings.layers.as_mut().expect("allocated")[0].bend_cost = Some(2.5);

    settings.apply_board_specific_optimizations(&board);

    let layers = settings.layers.as_ref().expect("allocated");
    assert_eq!(layers[0].bend_cost, Some(2.5));
    assert_eq!(
        layers[1].bend_cost,
        Some(99.0),
        "RouterSettings.java:357-360 copies defaultBendCost with no clamp"
    );
}

#[test]
fn a_short_layer_array_is_grown_by_index_and_keeps_its_entries() {
    let board = synthetic_board(2_000_000, 1_000_000, &[true, true, true, true]);
    let mut settings = RouterSettings::new();
    settings.layers = Some(vec![
        LayerSettings::with_bend_cost(Some(false), Some(true), Some(7.0)),
        LayerSettings::default(),
    ]);

    settings.apply_board_specific_optimizations(&board);

    assert_eq!(settings.get_layer_count(), 4);
    let layers = settings.layers.as_ref().expect("allocated");
    assert_eq!(layers[0].routable, Some(false));
    assert_eq!(layers[0].preferred_direction_horizontal, Some(true));
    assert_eq!(layers[0].bend_cost, Some(7.0));
    assert!(settings.are_board_specific_trace_costs_applied());
    assert_eq!(settings.get_preferred_direction_trace_costs(0), 1.8);
}

#[test]
fn jvm_golden_issue026_j2_reference() {
    let board = fixture_board("Issue026-J2_reference.dsn");
    assert_eq!(board.get_bounding_box().width(), 471_900);
    assert_eq!(board.get_bounding_box().height(), 256_000);
    assert_eq!(board.get_layer_count(), 2);

    let mut settings = sized_settings(2);
    settings.apply_board_specific_optimizations(&board);

    assert_eq!(
        rows(&settings),
        vec![(true, true, 0.0, 1.0, 2.8), (true, false, 0.0, 1.0, 1.5),]
    );
}

#[test]
fn jvm_golden_issue143_rpi_splitter() {
    let board = fixture_board("Issue143-rpi_splitter.dsn");
    assert_eq!(board.get_bounding_box().width(), 2_128_000);
    assert_eq!(board.get_bounding_box().height(), 4_192_000);

    let mut settings = sized_settings(board.get_layer_count());
    settings.apply_board_specific_optimizations(&board);

    assert_eq!(
        rows(&settings),
        vec![(true, false, 0.0, 1.0, 3.0), (true, true, 0.0, 1.0, 1.5),]
    );
}

#[test]
fn jvm_golden_issue145_smoothieboard() {
    let board = fixture_board("Issue145-smoothieboard.dsn");
    assert_eq!(board.get_bounding_box().width(), 1_297_480);
    assert_eq!(board.get_bounding_box().height(), 1_052_289);
    assert_eq!(board.get_layer_count(), 4);
    assert_eq!(board.layer_structure().signal_layer_count(), 3);

    let mut settings = sized_settings(4);
    settings.apply_board_specific_optimizations(&board);

    assert_eq!(
        rows(&settings),
        vec![
            (true, true, 0.0, 1.6, 2.800_000_000_000_000_3),
            (false, true, 0.0, 1.0, 2.2),
            (true, false, 0.0, 1.0, 1.8),
            (true, true, 0.0, 1.6, 2.800_000_000_000_000_3),
        ]
    );

    let before = rows(&settings);
    settings.apply_board_specific_optimizations_if_needed(&board);
    assert_eq!(rows(&settings), before);

    let mut bare = RouterSettings::new();
    bare.apply_board_specific_optimizations(&board);
    assert_eq!(rows(&bare), before);
}

#[test]
fn a_merged_settings_object_loses_the_flag_and_is_retuned() {
    let board = synthetic_board(2_000_000, 1_000_000, &[true, true]);
    let mut source = sized_settings(2);
    source.apply_board_specific_optimizations(&board);
    source.set_against_preferred_direction_trace_costs(0, 4.5);
    assert!(source.are_board_specific_trace_costs_applied());

    let mut merged = RouterSettings::new();
    let _ = merged.apply_new_values_from(&source);
    assert!(!merged.are_board_specific_trace_costs_applied());
    assert_eq!(merged.get_layer_count(), 2);
    assert_eq!(merged.get_against_preferred_direction_trace_costs(0), 4.5);

    merged.apply_board_specific_optimizations(&board);
    assert_eq!(
        merged.get_against_preferred_direction_trace_costs(0),
        3.0,
        "the user's 4.5 is gone: the flag did not survive the merge"
    );
}

#[test]
fn if_needed_re_tunes_on_a_layer_count_mismatch_even_when_the_flag_is_set() {
    let board = synthetic_board(1_000_000, 2_000_000, &[true, true]);
    let mut settings = sized_settings(4);
    settings.apply_board_specific_optimizations(&board);
    assert!(settings.are_board_specific_trace_costs_applied());
    settings.set_layer_count(4);
    settings.set_against_preferred_direction_trace_costs(0, 4.5);
    assert_eq!(settings.get_layer_count(), 4);
    assert!(settings.are_board_specific_trace_costs_applied());

    settings.apply_board_specific_optimizations_if_needed(&board);

    assert_eq!(settings.get_layer_count(), 2, "re-sized to the board");
    assert_eq!(
        rows(&settings),
        vec![(true, false, 0.0, 1.0, 3.0), (true, true, 0.0, 1.0, 1.5)],
        "the user's 4.5 was replaced by the board's numbers"
    );
    assert!(settings.are_board_specific_trace_costs_applied());
}

#[test]
fn a_wrong_length_cost_array_re_initializes_costs_even_when_the_flag_is_set() {
    let board = synthetic_board(2_000_000, 1_000_000, &[true, true]);
    let mut settings = sized_settings(2);
    settings.apply_board_specific_optimizations(&board);
    settings.set_against_preferred_direction_trace_costs(0, 4.5);
    assert!(settings.are_board_specific_trace_costs_applied());
    assert_eq!(settings.get_layer_count(), 2);

    let scoring = settings.scoring.as_mut().expect("allocated");
    scoring.undesired_direction_trace_cost = Some(vec![4.5]);
    assert_eq!(settings.get_layer_count(), 2);
    assert!(settings.are_board_specific_trace_costs_applied());

    settings.apply_board_specific_optimizations(&board);

    assert_eq!(
        rows(&settings),
        vec![(true, true, 0.0, 1.0, 3.0), (true, false, 0.0, 1.0, 1.5)],
        "the arrays were re-initialised, not preserved"
    );
    assert!(settings.are_board_specific_trace_costs_applied());
}
