mod board_builder;

use copper_board::items::Item;
use copper_geometry::{IntBox, TileShape};

#[test]
fn an_already_sufficient_named_class_is_left_unchanged() {
    let mut board = board_builder::p2t11_board();
    board.rules.clearance_matrix.set_default_value(200);
    board
        .rules
        .clearance_matrix
        .append_class("same-clearance-different-name");
    let class = board.rules.clearance_matrix.get_class_count() - 1;
    let pin = board.get_pins()[0];
    board.change_clearance_class_index(pin, class);
    let before = board.rules.clearance_matrix.clone();
    assert!(!board.raise_pin_clearance(pin, &[100, 0]));
    assert_eq!(board.get_item(pin).unwrap().clearance_class(), class);
    assert_eq!(board.rules.clearance_matrix, before);
}

#[test]
fn an_exposed_pad_blocks_foreign_copper_only_on_its_mask_layer() {
    let mut board = board_builder::p2t11_board();
    let pin = board
        .get_pins()
        .into_iter()
        .find(|id| matches!(board.get_item(*id), Some(Item::Pin(p)) if p.get_pin_index() == 1))
        .unwrap();
    let copper = TileShape::Box(IntBox::from_coords(1350, 980, 1370, 1020));
    for layer in [0, 1] {
        assert!(
            !board
                .overlapping_items_with_clearance(&copper, Some(layer), &[2], 1)
                .contains(&pin)
        );
    }
    let base = board.get_item(pin).unwrap().clearance_class();
    let forward = board.rules.clearance_matrix.get_value(base, 2, 0, false);
    let reverse = board.rules.clearance_matrix.get_value(2, base, 0, false);
    assert!(board.raise_pin_clearance(pin, &[400, 0]));
    assert!(
        board
            .overlapping_items_with_clearance(&copper, Some(0), &[2], 1)
            .contains(&pin)
    );
    assert!(
        !board
            .overlapping_items_with_clearance(&copper, Some(1), &[2], 1)
            .contains(&pin)
    );
    assert!(
        !board
            .overlapping_items_with_clearance(&copper, Some(0), &[1], 1)
            .contains(&pin)
    );
    assert_eq!(board.rules.clearance_matrix.get_value(1, 1, 0, false), 200);
    assert_eq!(board.rules.clearance_matrix.get_value(2, 1, 0, false), 600);
    let class = board.get_item(pin).unwrap().clearance_class();
    assert_eq!(
        board.rules.clearance_matrix.get_value(class, 2, 0, false),
        forward.max(400)
    );
    assert_eq!(
        board.rules.clearance_matrix.get_value(2, class, 0, false),
        reverse.max(400)
    );
    let matrix = board.rules.clearance_matrix.clone();
    assert!(!board.raise_pin_clearance(pin, &[400, 0]));
    assert_eq!(board.get_item(pin).unwrap().clearance_class(), class);
    assert_eq!(board.rules.clearance_matrix, matrix);
}

#[test]
fn mask_floors_preserve_an_escape_from_existing_foreign_pad_copper() {
    use copper_board::prelude::*;
    use copper_geometry::Point;
    for net in [1, 2] {
        let mut board = board_builder::p2t11_board();
        let pin = board
            .get_pins()
            .into_iter()
            .find(|id| matches!(board.get_item(*id), Some(Item::Pin(p)) if p.get_pin_index() == 0))
            .unwrap();
        let package = board.components.get(1).get_package();
        let component = board
            .components
            .add_with_generated_name(Some(Point::new(400, 0)), 0.0, true, package)
            .id;
        board.insert_pin(component, 0, vec![net], 1, FixedState::SystemFixed);
        let Item::Pin(pad) = board.items.get_mut(&pin).unwrap() else {
            unreachable!()
        };
        pad.solder_mask_expansion.insert(0, 400);
        assert!(board.raise_solder_mask_clearances());
        let class = board.get_item(pin).unwrap().clearance_class();
        let expected = if net == 1 { 400 } else { 300 };
        assert_eq!(
            board.rules.clearance_matrix.get_value(class, 1, 0, false),
            expected,
            "neighbor net={net}"
        );
        assert_eq!(
            board.rules.clearance_matrix.get_value(class, 1, 1, false),
            200
        );
        assert_eq!(
            board.rules.clearance_matrix.get_value(2, class, 0, false),
            600,
            "retain the stricter preexisting copper rule"
        );
    }
}
