mod common;

use common::synthetic::{PadSpec, SyntheticBoard};
use fr_drc::checks::geometry::{gap_below, hole_of, is_microvia, is_through_hole_pin, item_shapes};
use fr_geometry::{IntBox, IntVector, TileShape};

fn boxes(gap: i32) -> (TileShape, TileShape) {
    (
        TileShape::Box(IntBox::from_coords(0, 0, 1000, 1000)),
        TileShape::Box(IntBox::from_coords(1000 + gap, 0, 2000 + gap, 1000)),
    )
}

#[test]
fn gap_below_reports_the_actual_gap_only_when_it_is_under_the_clearance() {
    let (a, b) = boxes(300);
    assert!(gap_below(&a, &b, 300).is_none());
    let (actual, _) = gap_below(&a, &b, 500).expect("300 is under 500");
    assert!((actual - 300.0).abs() < 2.0, "actual {actual}");
    let (a, b) = boxes(-100);
    let (actual, _) = gap_below(&a, &b, 500).expect("overlap is under any clearance");
    assert_eq!(actual, 0.0);
    assert!(
        gap_below(&a, &b, 0).is_some(),
        "overlap is reported even at zero clearance"
    );
    let (a, b) = boxes(10);
    assert!(
        gap_below(&a, &b, 0).is_none(),
        "a gap is fine at zero clearance"
    );
}

#[test]
fn via_holes_are_exact_and_pad_holes_are_estimated() {
    let mut synthetic = SyntheticBoard::new(
        &[
            PadSpec {
                name: "1",
                half: 800,
                offset: IntVector::new(0, 0),
                through_hole: true,
            },
            PadSpec {
                name: "2",
                half: 800,
                offset: IntVector::new(5000, 0),
                through_hole: false,
            },
        ],
        1,
        2000,
    );
    let via = synthetic.via(20_000, 0, 1);
    let micro = synthetic.microvia(30_000, 0, 1);
    let th = synthetic.pin(0, 1);
    let smd = synthetic.pin(1, 1);
    let board = &synthetic.board;

    let hole = hole_of(board, via).expect("a via has a hole");
    assert!(!hole.estimated);
    assert!((hole.radius - 1500.0).abs() < 1.0, "radius {}", hole.radius);
    assert!(is_through_hole_pin(board, th));
    assert!(!is_through_hole_pin(board, smd));
    let pad_hole = hole_of(board, th).expect("a through-hole pin has a hole");
    assert!(pad_hole.estimated);
    assert!(hole_of(board, smd).is_none());
    assert!(!is_microvia(board, via));
    assert!(!is_microvia(board, micro));
}

#[test]
fn item_shapes_lists_one_shape_per_layer_for_a_via_and_one_for_a_trace() {
    let mut synthetic = SyntheticBoard::new(&[], 1, 2000);
    let via = synthetic.via(0, 0, 1);
    let trace = synthetic.trace(&[(0, 0), (10_000, 0)], 1, 500, 1);
    let via_shapes = item_shapes(&mut synthetic.board, via);
    assert_eq!(
        via_shapes
            .iter()
            .map(|(layer, _)| *layer)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    let trace_shapes = item_shapes(&mut synthetic.board, trace);
    assert_eq!(trace_shapes.len(), 1);
    assert_eq!(trace_shapes[0].0, 1);
}
