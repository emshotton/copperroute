mod board_builder;

use copper_board::items::{ConductionArea, ItemHeader, ObstacleAreaData};
use copper_board::prelude::*;
use copper_geometry::{Area, IntBox, Shape, TileShape, Vector};

fn stack(names: &[(&str, bool)]) -> LayerStructure {
    LayerStructure::new(
        names
            .iter()
            .map(|(name, is_signal)| Layer::new((*name).to_string(), *is_signal))
            .collect(),
    )
}

#[test]
fn an_out_of_range_signal_layer_is_none() {
    let four = stack(&[
        ("F.Cu", true),
        ("In1.Cu", false),
        ("In2.Cu", true),
        ("B.Cu", true),
    ]);
    assert_eq!(four.signal_layer_count(), 3);
    // In range, unchanged.
    assert_eq!(
        four.get_signal_layer(0).map(|l| l.name.as_str()),
        Some("F.Cu")
    );
    assert_eq!(
        four.get_signal_layer(2).map(|l| l.name.as_str()),
        Some("B.Cu")
    );
    assert_eq!(four.get_signal_layer(3), None);
    assert_eq!(four.get_signal_layer(99), None);

    let non_signal_last = stack(&[("F.Cu", true), ("Adhes", false)]);
    assert_eq!(non_signal_last.get_signal_layer(1), None);

    // The crash case: `layers[layers.length - 1]` with `length == 0`.
    assert_eq!(stack(&[]).get_signal_layer(0), None);

    // `getLayerNo(int)` is `getNo(getSignalLayer(n))` and inherits the answer rather than
    // laundering it back into an index.
    assert_eq!(four.get_layer_no_of_signal_layer(2), Some(3));
    assert_eq!(four.get_layer_no_of_signal_layer(3), None);
    assert_eq!(non_signal_last.get_layer_no_of_signal_layer(1), None);
    assert_eq!(stack(&[]).get_layer_no_of_signal_layer(0), None);
}

/// A conduction area on `net_nos`, built directly so the net count is the only variable.
fn area_on(net_nos: Vec<i32>) -> ConductionArea {
    ConductionArea::new(
        ItemHeader::new(ItemId(1), net_nos, 1, 0, FixedState::Unfixed),
        ObstacleAreaData::new(
            Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
                -100, -100, 100, 100,
            )))),
            0,
            Vector::ZERO,
            0.0,
            false,
            None,
        ),
        true,
    )
}

#[test]
fn a_zero_net_conduction_area_copies() {
    let none = area_on(Vec::new());
    let copied = none
        .copy(ItemId(2))
        .expect("an area on no nets copies to an area on no nets — quirk #46");
    assert_eq!(copied.hdr.net_count(), 0);
    assert_eq!(copied.hdr.id(), ItemId(2));
    assert_eq!(copied.get_is_obstacle(), none.get_is_obstacle());
    assert_eq!(copied.get_layer(), none.get_layer());

    // One net: unchanged, and the reference point for the copy above.
    let one = area_on(vec![7]);
    let copied = one.copy(ItemId(3)).expect("the one-net case always worked");
    assert_eq!(copied.hdr.net_nos, vec![7]);

    assert_eq!(area_on(vec![7, 8]).copy(ItemId(4)), None);
    assert_eq!(area_on(vec![7, 8, 9]).copy(ItemId(5)), None);
}
