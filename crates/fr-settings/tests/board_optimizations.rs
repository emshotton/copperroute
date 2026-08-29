//! `RouterSettings.applyBoardSpecificOptimizations` (`settings/RouterSettings.java:240-259,
//! 266-435`) — ports of `settings/Issue729TraceCostSettingsTest.java` and
//! `settings/RouterSettingsMergeTest.java:40-84`, plus three JVM goldens taken over real DSN
//! fixtures.
//!
//! # JVM goldens — the command
//!
//! Every expected number below (including the synthetic-board ones the two Java tests only
//! assert loosely) came out of `crates/fr-settings/tests/data/BProbe.java` run against the
//! clone-HEAD jar (plan ruling 7):
//!
//! ```sh
//! JAR=/Users/em/Development/freerouting/freerouting/build/libs/freerouting-current-executable.jar
//! ls -la "$JAR"   # 63 288 650 bytes, mtime 2026-08-27 20:03
//! F=/Users/em/Development/freerouting/freerouting/fixtures
//! /opt/homebrew/opt/openjdk@25/bin/javac -cp "$JAR" -d . BProbe.java
//! /opt/homebrew/opt/openjdk@25/bin/java -Djava.awt.headless=true -cp "$JAR:." BProbe \
//!     $F/Issue026-J2_reference.dsn $F/Issue143-rpi_splitter.dsn $F/Issue145-smoothieboard.dsn
//! ```
//!
//! The transcript is in `.superpowers/sdd/2026-08-28-plan-4-settings/task-5-report.md`.
//!
//! # Java-wins correction to the task brief
//!
//! The brief's last test says a `[signal, plane, signal]` stack leaves "layers 0 and 2 both …
//! horizontal". It does not, and the JVM says so: on a 2 000 000 × 1 000 000 board the running
//! flag starts `false`, layer 0 toggles it to `true`, the plane at layer 1 does **not** toggle
//! (`:349-351`) but still *inherits* the running `true` into its own
//! `preferredDirectionHorizontal` (`:361-363`, which is not signal-gated), and layer 2 toggles it
//! to `false`. So the plane shares layer 0's direction and layer 2 is vertical —
//! `BProbe` line `synthetic-sig-plane-sig-2000000x1000000/bare L2 … prefHoriz=false`. What the
//! non-signal layer really costs is the *toggle*, not the direction: without the signal gate
//! layer 2 would have come out horizontal.

use fr_board::prelude::*;
use fr_dsn::BoardReadResult;
use fr_dsn::parser::scope_parameter::DsnReadOptions;
use fr_geometry::{IntBox, PolylineShapeRef, TileShape};
use fr_settings::{LayerSettings, RouterSettings};

// ---------------------------------------------------------------------------------------------
// fixtures
// ---------------------------------------------------------------------------------------------

/// The board `Issue729TraceCostSettingsTest.setUp` (`@BeforeEach`, :28-47) and
/// `RouterSettingsMergeTest.applyBoardSpecificOptimizationsPreservesSettings` (:40-83, at its
/// :49-73) build: a
/// stack of named layers, a default clearance matrix at half-width 10, a default net class and an
/// `IntBox` outline that is also the bounding box.
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

/// A real board, read the way `RoutingJobScheduler` reads one before it calls
/// `applyBoardSpecificOptimizations` (`RoutingJobScheduler.java:186`).
fn fixture_board(name: &str) -> Board {
    // `parity::fixture`, not a `CARGO_MANIFEST_DIR`-relative literal: the latter cannot see
    // `FREEROUTING_JAVA_DIR` and breaks whenever the checkout is not a sibling of the Java clone
    // (a `git worktree`, for instance). Task 8 fix round 1.
    let path = parity::fixture(name);
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("cannot read fixture {}: {e}", path.display()));
    match fr_dsn::read_board(&bytes[..], None, Some(name), &DsnReadOptions::default()) {
        BoardReadResult::Success { board, .. } | BoardReadResult::OutlineMissing { board, .. } => {
            *board.unwrap_or_else(|| panic!("{name} produced no board"))
        }
        other => panic!("{name} did not read: {other:?}"),
    }
}

/// One row per layer: `(routable, preferred_direction_horizontal, bend_cost,
/// preferred_direction_trace_cost, undesired_direction_trace_cost)` — the five values `BProbe`
/// prints, read back through the accessors Task 4 ported.
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

/// `new RouterSettings(); setLayerCount(n)` — the `Issue729TraceCostSettingsTest` fixture.
fn sized_settings(layer_count: usize) -> RouterSettings {
    let mut settings = RouterSettings::new();
    settings.set_layer_count(layer_count);
    settings
}

// ---------------------------------------------------------------------------------------------
// Issue729TraceCostSettingsTest.java
// ---------------------------------------------------------------------------------------------

/// `Issue729TraceCostSettingsTest.applyBoardSpecificOptimizationsInitializesTraceCostsOnce`
/// (:49-64), tightened from the Java test's inequality to the exact JVM numbers.
///
/// `horizontal_add == 0.1 * java_round(10.0 * 2e6 / 1e6) == 2.0`,
/// `vertical_add == 0.1 * java_round(10.0 * 1e6 / 2e6) == 0.5` (`:299-303`). The running flag
/// starts `2e6 < 1e6 == false`, so layer 0 toggles to horizontal and layer 1 back to vertical.
/// `signalLayerCount() == 2`, so there is **no** outer-layer bonus (`:375-386`).
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

/// `Issue729TraceCostSettingsTest.applyBoardSpecificOptimizationsPreservesUserTraceCostsOnSecondCall`
/// (:66-79).
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

/// `Issue729TraceCostSettingsTest.applyBoardSpecificOptimizationsIfNeededSkipsWhenAlreadyBoardTuned`
/// (:81-91).
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

/// `Issue729TraceCostSettingsTest.applyBoardSpecificOptimizationsIfNeededRunsWhenLayerCountMismatch`
/// (:93-104).
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
    // BProbe's `bare` rows: identical to the `setLayerCount(2)` ones.
    assert_eq!(
        rows(&untuned),
        vec![(true, true, 0.0, 1.0, 3.0), (true, false, 0.0, 1.0, 1.5),]
    );
}

// ---------------------------------------------------------------------------------------------
// RouterSettingsMergeTest.java:40-83
// ---------------------------------------------------------------------------------------------

/// `RouterSettingsMergeTest.applyBoardSpecificOptimizationsPreservesSettings` (:40-83), on a
/// **square** 2 000 000 × 2 000 000 board — the only fixture in this suite where both penalties
/// are equal, `0.1 * java_round(10.0) == 1.0`.
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

    // The square board makes both aspect-ratio penalties 1.0, so both layers land on 2.0
    // whichever way the running flag points.
    assert_eq!(settings.get_against_preferred_direction_trace_costs(0), 2.0);
    assert_eq!(settings.get_against_preferred_direction_trace_costs(1), 2.0);
}

// ---------------------------------------------------------------------------------------------
// the two branches the Java tests never reach
// ---------------------------------------------------------------------------------------------

/// `:375-386` — four signal layers give `outer == 0.2 * 4 == 0.8`, added to index `0` and
/// `layerCount - 1` of **both** arrays.
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

/// `:349-353` — a non-signal layer is forced unroutable and does **not** advance the running
/// direction, but still inherits it. See the Java-wins note in this file's header: the plane
/// shares layer 0's horizontal direction, and layer 2 comes out *vertical*.
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

/// A user-supplied `bendCost` survives, and an absent one takes `scoring.defaultBendCost`
/// **unclamped** (`:357-360`) — unlike `setBendCost`, which clamps into
/// `[MIN_BEND_COST, MAX_BEND_COST]`.
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

/// `:306-318` — a layer array of the wrong length is reallocated element-by-element, keeping the
/// entries that fit and clearing the applied flag so the costs are recomputed.
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

// ---------------------------------------------------------------------------------------------
// JVM goldens over real DSN fixtures
// ---------------------------------------------------------------------------------------------

/// `Issue026-J2_reference.dsn`: 471 900 × 256 000, two signal layers.
/// `horizontal_add == 0.1 * java_round(18.43…) == 1.8`,
/// `vertical_add == 0.1 * java_round(5.42…) == 0.5`.
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

/// `Issue143-rpi_splitter.dsn`: 2 128 000 × 4 192 000 — **taller than wide**, so the running flag
/// starts `true` and layer 0 comes out vertical, the mirror image of every other fixture here.
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

/// `Issue145-smoothieboard.dsn`: 1 297 480 × 1 052 289, a four-layer `[signal, plane, signal,
/// signal]` stack — the one fixture that exercises the plane layer, the outer-layer bonus
/// (`signalLayerCount() == 3`, `outer == 0.2 * 3 == 0.6000000000000001`) and `f64` accumulation
/// order all at once. `2.8000000000000003 == 1.0 + 0.1 * 12.0 + 0.2 * 3.0` in that order; adding
/// the bonus first would give `2.8`, so this row pins `:365-373` running before `:379-384`.
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

    // `applyBoardSpecificOptimizationsIfNeeded` on the tuned object is a no-op, and a bare
    // `new RouterSettings()` reaches the same rows through the reallocation branch.
    let before = rows(&settings);
    settings.apply_board_specific_optimizations_if_needed(&board);
    assert_eq!(rows(&settings), before);

    let mut bare = RouterSettings::new();
    bare.apply_board_specific_optimizations(&board);
    assert_eq!(rows(&bare), before);
}

/// `docs/java-quirks.md` #127 (survey Q9): `copyFields` skips the `private`
/// `boardSpecificTraceCostsApplied` (`ReflectionUtil.java:226-228`), so a merged
/// `RouterSettings` inherits the source's *tuned* cost arrays with the flag back at "not
/// applied" — and `RoutingJobScheduler.java:186`'s `applyBoardSpecificOptimizations` then
/// overwrites the very costs the merge just carried across.
///
/// JVM-verified (`BProbe` block Q9): `Q9.source applied=true und0=4.5`,
/// `Q9.merged applied=false layerCount=2 und0=4.5`, `Q9.retuned applied=true und0=3.0`.
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

/// The **first** disjunct of `applyBoardSpecificOptimizationsIfNeeded` (`:252-253`) on its own:
/// the flag says the costs *are* board-tuned, but the layer count disagrees with the board's, so
/// the guard still fires. `Issue729TraceCostSettingsTest` covers the pair
/// (`…SkipsWhenAlreadyBoardTuned` has both disjuncts false, `…RunsWhenLayerCountMismatch` has
/// both true); neither isolates `getLayerCount() != boardLayerCount && applied`.
///
/// The re-tune is visible twice over: `layers` is reallocated to the board's length
/// (`:306-318`), which itself clears the flag, and the user's `4.5` is replaced by the board's
/// own number. On a **tall** 1 000 000 × 2 000 000 board the running flag starts
/// `1e6 < 2e6 == true`, layer 0 toggles it to `false`, so layer 0's undesired direction is the
/// horizontal one and its penalty is `vertical_add == 0.1 * java_round(10.0 * 2e6 / 1e6) == 2.0`
/// (`:299-303`, `:365-373`) — `1.0 + 2.0 == 3.0`.
#[test]
fn if_needed_re_tunes_on_a_layer_count_mismatch_even_when_the_flag_is_set() {
    let board = synthetic_board(1_000_000, 2_000_000, &[true, true]);
    let mut settings = sized_settings(4);
    settings.apply_board_specific_optimizations(&board);
    assert!(settings.are_board_specific_trace_costs_applied());
    // The first pass already re-sized to the board; put the mismatch back by hand, keeping the
    // flag set, which is the state the second disjunct alone cannot produce.
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

/// `applyBoardSpecificOptimizations`' cost-array branch (`:325-334`) reached with the flag
/// **already true** and `layers` already the right length: only the two `double[]`s are the
/// wrong size, and reallocating either one writes `boardSpecificTraceCostsApplied = false`
/// (`:328`, `:333`) — *before* `:346` reads it into `initializeTraceCosts`. So a settings object
/// whose costs were "already applied" has them recomputed anyway, purely because the arrays did
/// not match the layer count.
///
/// This is the one path where the flag is cleared by something other than the `layers`
/// reallocation, and the port collapses Java's two identical writes into one after the pair
/// (the borrow of `scoring` forces it); the assertion is that the collapse is invisible.
#[test]
fn a_wrong_length_cost_array_re_initializes_costs_even_when_the_flag_is_set() {
    let board = synthetic_board(2_000_000, 1_000_000, &[true, true]);
    let mut settings = sized_settings(2);
    settings.apply_board_specific_optimizations(&board);
    settings.set_against_preferred_direction_trace_costs(0, 4.5);
    assert!(settings.are_board_specific_trace_costs_applied());
    assert_eq!(settings.get_layer_count(), 2);

    // Shorten one cost array only. `layers` stays at the board's length, so `:306-318` does not
    // fire and the flag survives everything but `:325-334`.
    let scoring = settings.scoring.as_mut().expect("allocated");
    scoring.undesired_direction_trace_cost = Some(vec![4.5]);
    assert_eq!(settings.get_layer_count(), 2);
    assert!(settings.are_board_specific_trace_costs_applied());

    settings.apply_board_specific_optimizations(&board);

    // `2e6 x 1e6`: horizontal penalty `2.0`, vertical `0.5`; layer 0 is horizontal.
    assert_eq!(
        rows(&settings),
        vec![(true, true, 0.0, 1.0, 3.0), (true, false, 0.0, 1.0, 1.5)],
        "the arrays were re-initialised, not preserved"
    );
    assert!(settings.are_board_specific_trace_costs_applied());
}
