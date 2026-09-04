//! Plan 9 Task 10: the two "answers nothing where it should answer something" rows, #35 and #46.
//!
//! Both are cases where Java produced a value nobody could use and no caller was told:
//!
//! * **#35** — `LayerStructure.getSignalLayer(no)` (`LayerStructure.java:49-61`) falls off its
//!   loop for an out-of-range `no` and returns `layers[layers.length - 1]`, the last layer of the
//!   **whole stack**, signal or not; and on an empty stack it throws
//!   `ArrayIndexOutOfBoundsException` on `layers[-1]`. A silent wrong answer and a crash, from one
//!   method.
//! * **#46** — `ConductionArea.copy(int)` (`:309-330`) returns **`null`** whenever
//!   `netCount() != 1`, after a warning that says "more than 1 net". Zero nets is not more than
//!   one, and every other `Item.copy` override always produces an item, so a caller that does not
//!   null-check gets an NPE one call later.
//!
//! Both are `Option`-returning in the port, so the answer has to be looked at either way; what
//! Plan 9 Task 10 changes is *which* inputs answer `None`.

mod board_builder;

use fr_board::items::{ConductionArea, ItemHeader, ObstacleAreaData};
use fr_board::prelude::*;
use fr_geometry::{Area, IntBox, Shape, TileShape, Vector};

fn stack(names: &[(&str, bool)]) -> LayerStructure {
    LayerStructure::new(
        names
            .iter()
            .map(|(name, is_signal)| Layer::new((*name).to_string(), *is_signal))
            .collect(),
    )
}

/// Quirk #35, fixed at Plan 9 Task 10.
///
/// Three inputs, one answer. The middle one is why the fallback was a *wrong* answer rather than
/// an odd one: on a stack whose last layer is not a signal layer, Java handed back a non-signal
/// layer in answer to "give me the n-th signal layer".
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
    // One past the end, and far past it. Java answered `B.Cu` to both.
    assert_eq!(four.get_signal_layer(3), None);
    assert_eq!(four.get_signal_layer(99), None);

    // The wrong-answer case: the last layer of the stack is not a signal layer at all, so Java's
    // fallback answered a non-signal layer to "the n-th signal layer".
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

/// Quirk #46, fixed at Plan 9 Task 10 — the half the sketch says "needs no new logic at all".
///
/// Java refuses `netCount() != 1` behind a warning that says "more than 1 net". Zero nets is not
/// more than one, and an area on no nets copies to an area on no nets: the header copy already
/// carries the (empty) net list. The guard is `> 1` now, so only the case Java's own warning names
/// still answers `None`.
///
/// The multi-net copy is deliberately **not** implemented here, and the test says so rather than
/// leaving the reader to guess: nothing in the port constructs a multi-net conduction area, so
/// there is no way to measure the change, and "the constructor already takes an `int[]`" is an
/// argument rather than evidence. The register row carries it as an open question.
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

    // More than one: still `None`, which is what Java's own warning describes and what this fix
    // deliberately leaves alone.
    assert_eq!(area_on(vec![7, 8]).copy(ItemId(4)), None);
    assert_eq!(area_on(vec![7, 8, 9]).copy(ItemId(5)), None);
}
