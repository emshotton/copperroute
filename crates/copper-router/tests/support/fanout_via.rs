use copper_board::prelude::*;
use copper_geometry::{IntBox, Point, Polyline, Shape, TileShape};

/// An unfinished fanout with one trace contact, independent of route quality.
/// Reversing the polyline exercises both trace-end orientations.
pub fn board(reverse: bool) -> (Board, ItemId) {
    let layers = LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)]);
    let matrix = ClearanceMatrix::get_default_instance(&layers, 100);
    let mut rules = BoardRules::new(layers.clone(), matrix);
    rules.trace_angle_restriction = AngleRestriction::None;
    let mut padstacks = Padstacks::new(layers);
    let shape = Shape::Tile(TileShape::Box(IntBox::from_coords(-20, -20, 20, 20)));
    padstacks.add("via", vec![Some(shape.clone()), Some(shape)], true, false);
    let mut board = Board::new(
        Vec::new(),
        0,
        IntBox::from_coords(-10_000, -10_000, 10_000, 10_000),
        rules,
        BoardLibrary::new(padstacks, Packages::new()),
        Components::new(),
        Communication::default(),
    );
    let class = board.rules.get_default_net_class();
    board.rules.nets.add("N1", 1, false, class);
    let via = board
        .insert_via(
            PadstackId(1),
            Point::new(0, 0),
            vec![1],
            1,
            FixedState::Unfixed,
            false,
        )
        .unwrap();
    let mut points = vec![
        Point::new(0, 0),
        Point::new(1000, 0),
        Point::new(1000, 1000),
    ];
    if reverse {
        points.reverse();
    }
    board.insert_trace_without_cleaning(
        Polyline::from_points(&points),
        0,
        10,
        vec![1],
        1,
        FixedState::Unfixed,
    );
    (board, via)
}
