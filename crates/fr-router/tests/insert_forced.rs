//! Plan 9 Task 6, quirk #185: `RoutingBoard.insertForcedTracePolyline`'s null `newTrace`.
//!
//! `board/facade/RoutingBoard.java:756` calls `newTrace.combine()` with no null test and `:791`
//! — 35 lines later, on the same variable — guards it with `newTrace != null &&`. The register
//! (docs/java-quirks.md #185) records the defect as latent on all 1 621 rows of
//! `tests/data/p6t15b-insert-forced.txt`, so this file carries the synthetic fixture that
//! reaches it: `tests/data/t6-resampled-polyline.txt`, whose derivation this test executes.
//!
//! The headline is what the crash costs: no `catch` covers `:756` (the method's only `try` opens
//! at `:787`), so the nearest handler is `AutorouteConnectionRouter.route:155-158`'s bare
//! `FAILED` — **the whole connection is abandoned where the guard skips one segment**.

use fr_board::prelude::*;
use fr_geometry::{IntBox, IntPoint, Point, Polyline};
use fr_router::board_ext::RoutingBoardExt;

const BOUNDING_BOX: IntBox = IntBox {
    ll: IntPoint {
        x: -10_000,
        y: -10_000,
    },
    ur: IntPoint {
        x: 10_000,
        y: 10_000,
    },
};

fn layers() -> LayerStructure {
    LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)])
}

/// `tests/data/t6-resampled-polyline.txt`, "The board".
fn resampled_polyline_board() -> Board {
    let mut clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 200);
    assert!(clearance_matrix.append_class("wide"));
    clearance_matrix.set_value_on_all_layers(2, 1, 600);
    clearance_matrix.set_value_on_all_layers(2, 2, 800);
    let mut rules = BoardRules::new(layers(), clearance_matrix);
    rules.trace_angle_restriction = AngleRestriction::None;

    let mut board = Board::new(
        Vec::new(),
        0,
        BOUNDING_BOX,
        rules,
        BoardLibrary::new(Padstacks::new(layers()), Packages::new()),
        Components::new(),
        Communication::default(),
    );
    let default_class = board.rules.get_default_net_class();
    board.rules.nets.add("N1", 1, false, default_class);
    board.rules.nets.add("N2", 1, false, default_class);

    // Trace A: the only reason it exists is `BasicBoard.java:197-200`, which drops
    // `minTraceHalfWidth` to 30 and so makes `sampleWidth = 60` at RoutingBoard.java:641-644.
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(-5000, -5000), Point::new(-5000, -4000)]),
        0,
        30,
        vec![2],
        1,
        FixedState::Unfixed,
    );
    // Trace B: the obstacle across the *second* shove shape, and nothing else.
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(400, -429), Point::new(400, 771)]),
        0,
        30,
        vec![2],
        1,
        FixedState::Unfixed,
    );
    board
}

/// The polyline of `tests/data/t6-resampled-polyline.txt`, "The insertion".
fn resampled_polyline() -> Polyline {
    Polyline::from_points(&[
        Point::new(55, 24),
        Point::new(0, 0),
        Point::new(700, 300),
        Point::new(700, 1000),
    ])
}

/// The arithmetic of `tests/data/t6-resampled-polyline.txt`, re-derived **standalone**.
///
/// This calls [`Polyline::shorten`] directly with the arguments the derivation computes for
/// `:659-661` — it does **not** go through `insert_forced_trace_polyline`, and is not an
/// end-to-end check; the test above is that. What it is for is the half of the fixture that does
/// not depend on the fix: if a later change stops the method *reaching* `:756` with a closed
/// polyline, the test above would go quietly vacuous, and this one says which link of the
/// derivation broke.
#[test]
fn the_fixtures_shorten_closes_the_polyline_standalone() {
    let board = resampled_polyline_board();
    assert_eq!(
        board.get_min_trace_half_width(),
        30,
        "sampleWidth is 2 * this (RoutingBoard.java:641-644)"
    );

    let polyline = resampled_polyline();
    assert_eq!(polyline.lines().len(), 5);
    assert_eq!(polyline.corner_count(), 4);

    // `:659-661`'s newLineCount for lastShapeNo = 1 over three trace shapes: 5 - (3 - 1 - 1) = 4.
    let shortened = polyline
        .shorten(4, 60.0)
        .expect("the shorten itself is fine — it is what it produces that `:756` cannot take");
    assert_eq!(shortened.corner_count(), 2);
    assert_eq!(shortened.first_corner(), Some(Point::new(55, 24)));
    assert_eq!(
        shortened.last_corner(),
        shortened.first_corner(),
        "the resample has brought the polyline's two ends together, so \
         BasicBoard.java:191-195 refuses the trace and `newTrace` is null"
    );
}

/// Quirk #185, both sides. Before the fix this panics at `:756`; after it, the segment is skipped
/// and the caller is handed `newCorner` — the connection survives.
///
/// The literals are `tests/data/t6-resampled-polyline.txt`, "What the two sides answer".
#[test]
fn a_degenerate_resample_skips_one_segment_not_the_connection() {
    let never: &dyn Fn() -> bool = &|| false;
    let mut board = resampled_polyline_board();
    let items_before = board.items.len();

    let result = board
        .insert_forced_trace_polyline(
            None,
            &resampled_polyline(),
            30,
            0,
            &[1],
            1,
            0,
            0,
            0,
            i32::MAX,
            500,
            true,
            None,
            never,
        )
        .expect("no stop check trips here");

    // `:875` answers `newCorner`, which the resample moved to the polyline's own first corner.
    assert_eq!(
        result,
        Some(Point::new(55, 24)),
        "the guard skips the combine and the method runs on to `:875`; Java's unguarded `:756` \
         would have thrown out to a bare FAILED for the whole connection"
    );
    // The closed polyline was refused by `insertTraceWithoutCleaning` — that is
    // BasicBoard.java:191-195's own decision and is *not* what the fix changes, so no net-1 trace
    // reached the board.
    assert_eq!(
        board
            .items
            .values()
            .filter(|item| item.header().nets_equal(&[1]))
            .count(),
        0,
        "the refused trace is Java's decision at BasicBoard.java:191-195, not the guard's"
    );
    assert!(
        board.items.len() >= items_before,
        "the shove may have split trace B, but nothing was lost"
    );
}
