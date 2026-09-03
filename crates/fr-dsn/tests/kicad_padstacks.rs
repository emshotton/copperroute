//! **Padstack identity** on the KiCad board-JSON path: which pads share a padstack, and what
//! happens to a pad that has no shape on any layer.
//!
//! Java keys a pad's padstack on the name `getDescriptivePadstackName` generates
//! (`KiCadJsonReader.java:560-564`), looks it up case-insensitively (`Padstacks.java:25-32`), and
//! that name encodes **neither the layer span nor the drill** — so the second pad to generate a
//! name silently inherits the first pad's shapes and its `attachAllowed`. It also masks the
//! negative `DrillItem.tileShapeCount`: a pad whose `layers` match nothing borrows a valid
//! padstack instead of producing the all-`null` one that crashes.
//!
//! fixed: T7 (#284, #286).

use fr_board::{Board, PadstackId};
use fr_dsn::error::BoardReadResult;
use fr_dsn::kicad::read_board;

/// The board a payload loads to, or a panic naming what came back instead.
fn board(json: &str) -> Board {
    match read_board(json, None) {
        BoardReadResult::Success { board: Some(b), .. } => *b,
        other => panic!("expected a loaded board, got {other:?}"),
    }
}

/// The `(location, detail)` of a [`BoardReadResult::ParseError`].
fn parse_error(json: &str) -> (String, String) {
    match read_board(json, None) {
        BoardReadResult::ParseError { location, detail } => (location, detail),
        other => panic!("expected a ParseError, got {other:?}"),
    }
}

/// The padstack the first pin of component `index` (0-based, in `Components` order) sits on.
fn first_pin_padstack(board: &Board, index: usize) -> PadstackId {
    let component_package = board
        .components
        .get(i32::try_from(index).expect("a small index") + 1)
        .get_package();
    board
        .library
        .packages
        .get(component_package)
        .get_pin(0)
        .expect("the package has a pin")
        .padstack_no
}

/// A four-layer board whose two components carry one `1 x 1 mm` rectangular pad each, on the
/// layers `first` and `second` respectively.
fn two_pads_on(first: &str, second: &str, drill_a: f64, drill_b: f64) -> String {
    format!(
        r#"{{
        "layers":[{{"name":"F.Cu"}},{{"name":"In1.Cu"}},{{"name":"In2.Cu"}},{{"name":"B.Cu"}}],
        "components":[
          {{"reference":"U1","footprint":"FA","position":{{"x":0,"y":0}},
            "pads":[{{"name":"1","shape":"rect","size":{{"x":1.0,"y":1.0}},
                     "drill":{drill_a},"layers":[{first}]}}]}},
          {{"reference":"U2","footprint":"FB","position":{{"x":10,"y":0}},
            "pads":[{{"name":"1","shape":"rect","size":{{"x":1.0,"y":1.0}},
                     "drill":{drill_b},"layers":[{second}]}}]}}
        ]}}"#
    )
}

/// Two pads whose `layers` lists have the same **length** — so both spell their layer type `A` —
/// and different **contents**. Java gives them one padstack, and the second pad ends up with
/// copper on the first pad's layers.
#[test]
fn two_pads_of_the_same_name_and_different_layers_get_different_padstacks() {
    let board = board(&two_pads_on(
        r#""F.Cu","In1.Cu""#,
        r#""In2.Cu","B.Cu""#,
        0.0,
        0.0,
    ));
    let first = first_pin_padstack(&board, 0);
    let second = first_pin_padstack(&board, 1);
    assert_ne!(
        first, second,
        "the two pads span different layers and must not share a padstack"
    );

    let first = board.library.padstacks.get(first).expect("padstack 1");
    let second = board.library.padstacks.get(second).expect("padstack 2");
    assert_eq!(
        (first.from_layer(), first.to_layer()),
        (0, 1),
        "the F.Cu/In1.Cu pad spans layers 0..1"
    );
    assert_eq!(
        (second.from_layer(), second.to_layer()),
        (2, 3),
        "the In2.Cu/B.Cu pad spans layers 2..3, not the first pad's 0..1"
    );
}

/// The same two pads on the **same** layers: one padstack, because the identity is the shapes and
/// the drill and both agree.
#[test]
fn two_pads_of_the_same_shapes_and_drill_share_one_padstack() {
    let board = board(&two_pads_on(
        r#""F.Cu","In1.Cu""#,
        r#""F.Cu","In1.Cu""#,
        0.0,
        0.0,
    ));
    assert_eq!(
        first_pin_padstack(&board, 0),
        first_pin_padstack(&board, 1),
        "identical pads still share a padstack — the fix narrows the key, it does not abolish it"
    );
}

/// A drilled and an undrilled pad of the same size. Java's name carries no drill, so the second
/// pad inherited the first one's `attachAllowed` — the flag that decides whether a via of the
/// same net may overlap the pad.
#[test]
fn a_drilled_and_an_undrilled_pad_do_not_share_attach_allowed() {
    let board = board(&two_pads_on(
        r#""F.Cu","In1.Cu""#,
        r#""F.Cu","In1.Cu""#,
        0.5,
        0.0,
    ));
    let drilled = board
        .library
        .padstacks
        .get(first_pin_padstack(&board, 0))
        .expect("padstack 1");
    let undrilled = board
        .library
        .padstacks
        .get(first_pin_padstack(&board, 1))
        .expect("padstack 2");
    assert!(drilled.attach_allowed, "the drilled pad allows attachment");
    assert!(
        !undrilled.attach_allowed,
        "the undrilled pad must not inherit the drilled one's attachAllowed"
    );
}

/// `getDescriptivePadstackName`'s `"Round"` branch used the **one-number** `Pad_<x>_um` form, so
/// a `1 x 2 mm` round pad and a `1 x 1 mm` one had the same name.
#[test]
fn a_round_pad_name_carries_size_y() {
    let board = board(
        r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
            "components":[{"reference":"U1","footprint":"F","position":{"x":0,"y":0},
              "pads":[{"name":"1","shape":"circle","size":{"x":1.0,"y":2.0},"layers":["F.Cu"]}]}]}"#,
    );
    let padstack = board
        .library
        .padstacks
        .get(first_pin_padstack(&board, 0))
        .expect("the pad's padstack");
    assert_eq!(padstack.name, "Round[T]Pad_1000x2000_um");
}

/// A pad whose `layers` list names no layer this board has. Java left `startLayer > endLayer`, so
/// every shape stayed `null`, `DrillItem.tileShapeCount` answered `-layerCount`, and `insertPin`
/// allocated `new TileShape[-2]` — reported to the user as the bare number, `Exception occurred:
/// -2`.
#[test]
fn an_all_null_shape_array_is_refused_naming_the_pad() {
    let (location, detail) = parse_error(
        r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
            "components":[{"reference":"U7","footprint":"F","position":{"x":0,"y":0},
              "pads":[{"name":"3","shape":"rect","size":{"x":1.0,"y":1.0},
                       "layers":["Nonexistent.Cu"]}]}]}"#,
    );
    assert_eq!(location, "components");
    assert!(
        detail.contains("KiCad board JSON file"),
        "the diagnostic names the file: {detail}"
    );
    assert!(
        detail.contains("pad `3` of component `U7`"),
        "the diagnostic names the pad: {detail}"
    );
    assert!(
        detail.contains("no shape on any layer"),
        "the diagnostic says what is wrong: {detail}"
    );
    assert!(
        !detail.contains("-2"),
        "the bare NegativeArraySizeException number is gone: {detail}"
    );
}

/// The same refusal from the other direction: a via whose `startLayerIndex` is past its
/// `endLayerIndex`. Java put the via in the item list and *then* threw from inside the
/// search-tree update, leaving an item in no search tree.
#[test]
fn a_via_with_an_empty_layer_span_is_refused_naming_the_via() {
    let (location, detail) = parse_error(
        r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
            "vias":[{"netName":"GND","position":{"x":1,"y":1},"diameter":0.6,"drill":0.3,
                     "startLayerIndex":1,"endLayerIndex":0}]}"#,
    );
    assert_eq!(location, "vias");
    assert!(
        detail.contains("via `Via[1-0]_600:300_um`") && detail.contains("no shape on any layer"),
        "the diagnostic names the via and what is wrong: {detail}"
    );
}
