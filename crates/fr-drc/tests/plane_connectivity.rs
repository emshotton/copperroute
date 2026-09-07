mod common;

use common::synthetic::{PadSpec, SyntheticBoard};
use fr_board::prelude::*;
use fr_drc::PlaneConnectivity;
use fr_geometry::{Area, IntBox, IntVector, Shape, TileShape};

const GND: i32 = 1;
const SIGNAL: i32 = 2;
const CLEARANCE: i32 = 2000;
const SIGNAL_HALF_WIDTH: i32 = 1000;

const PADS: &[PadSpec] = &[
    PadSpec {
        name: "left",
        half: 1500,
        offset: IntVector { x: -30_000, y: 0 },
        through_hole: true,
    },
    PadSpec {
        name: "right",
        half: 1500,
        offset: IntVector { x: 30_000, y: 0 },
        through_hole: true,
    },
    PadSpec {
        name: "top-only",
        half: 1500,
        offset: IntVector { x: 0, y: -30_000 },
        through_hole: false,
    },
];

fn board_with_pour(layers: &[usize]) -> SyntheticBoard {
    let mut synthetic = SyntheticBoard::new(PADS, 2, CLEARANCE);
    for layer in layers {
        synthetic.board.insert_conduction_area(
            Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
                -40_000, -40_000, 40_000, 40_000,
            )))),
            *layer,
            vec![GND],
            1,
            false,
            FixedState::SystemFixed,
        );
    }
    synthetic.pin(0, GND);
    synthetic.pin(1, GND);
    synthetic
}

fn wall(synthetic: &mut SyntheticBoard, from_y: i32, to_y: i32) {
    synthetic.trace(&[(0, from_y), (0, to_y)], 0, SIGNAL_HALF_WIDTH, SIGNAL);
}

#[test]
fn boards_without_planes_have_nothing_to_check() {
    let mut synthetic = SyntheticBoard::new(PADS, 2, CLEARANCE);
    synthetic.pin(0, GND);
    synthetic.pin(1, GND);

    let connectivity = PlaneConnectivity::of(&synthetic.board);

    assert!(connectivity.is_empty());
    assert_eq!(connectivity.missing_connection_count(), 0);
    assert_eq!(connectivity.cluster_count(GND), None);
}

#[test]
fn two_pads_inside_one_pour_form_one_cluster() {
    let synthetic = board_with_pour(&[0]);

    let connectivity = PlaneConnectivity::of(&synthetic.board);

    assert_eq!(connectivity.cluster_count(GND), Some(1));
    assert_eq!(connectivity.missing_connection_count(), 0);
}

#[test]
fn a_signal_trace_crossing_the_whole_pour_splits_it() {
    let mut synthetic = board_with_pour(&[0]);
    wall(&mut synthetic, -45_000, 45_000);

    let connectivity = PlaneConnectivity::of(&synthetic.board);

    assert_eq!(connectivity.cluster_count(GND), Some(2));
    assert_eq!(connectivity.missing_connection_count(), 1);
}

#[test]
fn a_gap_wide_enough_for_the_minimum_pour_width_keeps_the_pour_connected() {
    let mut synthetic = board_with_pour(&[0]);
    wall(&mut synthetic, -45_000, -7_000);
    wall(&mut synthetic, 7_000, 45_000);

    let connectivity = PlaneConnectivity::of(&synthetic.board);

    assert_eq!(connectivity.cluster_count(GND), Some(1));
}

#[test]
fn a_gap_narrower_than_clearance_plus_pour_width_splits_the_pour() {
    let mut synthetic = board_with_pour(&[0]);
    wall(&mut synthetic, -45_000, -2_000);
    wall(&mut synthetic, 2_000, 45_000);

    let connectivity = PlaneConnectivity::of(&synthetic.board);

    assert_eq!(connectivity.cluster_count(GND), Some(2));
}

#[test]
fn a_pour_on_the_other_layer_bridges_a_split() {
    let mut synthetic = board_with_pour(&[0, 1]);
    wall(&mut synthetic, -45_000, 45_000);

    let connectivity = PlaneConnectivity::of(&synthetic.board);

    assert_eq!(connectivity.cluster_count(GND), Some(1));
}

#[test]
fn a_same_net_trace_on_the_other_layer_bridges_a_split() {
    let mut synthetic = board_with_pour(&[0]);
    wall(&mut synthetic, -45_000, 45_000);
    synthetic.trace(&[(-30_000, 0), (30_000, 0)], 1, SIGNAL_HALF_WIDTH, GND);

    let connectivity = PlaneConnectivity::of(&synthetic.board);

    assert_eq!(connectivity.cluster_count(GND), Some(1));
}

#[test]
fn a_top_only_pad_is_isolated_when_the_pour_is_cut_in_front_of_it() {
    let mut synthetic = board_with_pour(&[0, 1]);
    synthetic.pin(2, GND);
    synthetic.trace(
        &[(-45_000, -20_000), (45_000, -20_000)],
        0,
        SIGNAL_HALF_WIDTH,
        SIGNAL,
    );

    let connectivity = PlaneConnectivity::of(&synthetic.board);

    assert_eq!(connectivity.cluster_count(GND), Some(2));
}

#[test]
fn splitting_a_pour_further_counts_as_worse() {
    let mut synthetic = board_with_pour(&[0]);
    let before = PlaneConnectivity::of(&synthetic.board);
    wall(&mut synthetic, -45_000, 45_000);
    let after = PlaneConnectivity::of(&synthetic.board);

    assert!(after.splits_more_than(&before));
    assert!(!before.splits_more_than(&after));
    assert!(!after.splits_more_than(&after));
}

#[test]
fn a_smaller_pour_nested_in_another_net_pour_keeps_its_own_copper() {
    const RAIL: i32 = 2;
    let mut synthetic = board_with_pour(&[0]);
    synthetic.board.insert_conduction_area(
        Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
            -20_000, 10_000, 20_000, 30_000,
        )))),
        0,
        vec![RAIL],
        1,
        false,
        FixedState::SystemFixed,
    );
    synthetic.trace(
        &[(-15_000, 20_000), (-14_000, 20_000)],
        0,
        SIGNAL_HALF_WIDTH,
        RAIL,
    );
    synthetic.trace(
        &[(14_000, 20_000), (15_000, 20_000)],
        0,
        SIGNAL_HALF_WIDTH,
        RAIL,
    );

    let connectivity = PlaneConnectivity::of(&synthetic.board);

    assert_eq!(connectivity.cluster_count(RAIL), Some(1));
    assert_eq!(connectivity.cluster_count(GND), Some(1));
}
