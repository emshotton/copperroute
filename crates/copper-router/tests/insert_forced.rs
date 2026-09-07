use copper_board::prelude::*;
use copper_geometry::{IntBox, IntPoint, Point, Polyline};
use copper_router::board_ext::RoutingBoardExt;

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

    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(-5000, -5000), Point::new(-5000, -4000)]),
        0,
        30,
        vec![2],
        1,
        FixedState::Unfixed,
    );
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

fn resampled_polyline() -> Polyline {
    Polyline::from_points(&[
        Point::new(55, 24),
        Point::new(0, 0),
        Point::new(700, 300),
        Point::new(700, 1000),
    ])
}

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

    assert_eq!(
        result,
        Some(Point::new(55, 24)),
        "the guard skips the combine and the method runs on to `:875`; Java's unguarded `:756` \
         would have thrown out to a bare FAILED for the whole connection"
    );
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
