use copper_board::ids::PadstackId;
use copper_board::rules::clearance_matrix::CLEARANCE_SAFETY_MARGIN;
use copper_board::rules::{ClearanceMatrix, Nets, ViaInfo, ViaRule};
use copper_board::structure::{Layer, LayerStructure};

fn two_layer_structure() -> LayerStructure {
    LayerStructure::new(vec![Layer::new("Top", true), Layer::new("Bottom", true)])
}

#[test]
fn set_value_matches_java_test() {
    let layer_structure = two_layer_structure();
    let mut matrix = ClearanceMatrix::new(1, &layer_structure, &["default"]);

    matrix.set_value(0, 0, 0, 5);
    assert_eq!(matrix.get_value(0, 0, 0, false), 6);
    assert_eq!(matrix.max_value(0, 0), 6);
    assert_eq!(matrix.max_value_on_layer(0), 6);

    matrix.set_value(0, 0, 0, -10);
    assert_eq!(matrix.get_value(0, 0, 0, false), 0);
    assert_eq!(matrix.max_value(0, 0), 6);
    assert_eq!(matrix.max_value_on_layer(0), 6);

    matrix.set_value(0, 0, 0, i32::MAX);
    assert_eq!(matrix.get_value(0, 0, 0, false), i32::MAX - 1);
    assert_eq!(matrix.max_value(0, 0), i32::MAX - 1);
    assert_eq!(matrix.max_value_on_layer(0), i32::MAX - 1);
}

#[test]
fn get_value_uses_j_then_i_indexing() {
    let layer_structure = two_layer_structure();
    let mut matrix = ClearanceMatrix::new(3, &layer_structure, &["null", "default", "power"]);

    matrix.set_value(1, 2, 0, 100);
    assert_eq!(matrix.get_value(1, 2, 0, false), 100);
    assert_eq!(matrix.get_value(2, 1, 0, false), 0);
    assert_eq!(matrix.get_value(1, 2, 1, false), 0);

    assert_eq!(matrix.max_value(2, 0), 100);
    assert_eq!(matrix.max_value(1, 0), 0);
}

#[test]
fn safety_margin_adds_16() {
    assert_eq!(CLEARANCE_SAFETY_MARGIN, 16);

    let layer_structure = two_layer_structure();
    let mut matrix = ClearanceMatrix::new(2, &layer_structure, &["null", "default"]);
    matrix.set_value(1, 1, 0, 40);

    assert_eq!(matrix.get_value(1, 1, 0, false), 40);
    assert_eq!(matrix.get_value(1, 1, 0, true), 56);
    assert_eq!(matrix.get_value(0, 0, 0, true), 16);
    assert_eq!(matrix.get_value(2, 1, 0, true), 0);
    assert_eq!(matrix.get_value(1, 1, 2, true), 0);
}

#[test]
fn nets_hidden_net_constant() {
    assert_eq!(Nets::MAX_LEGAL_NET_NUMBER, 9_999_999);
    assert_eq!(Nets::HIDDEN_NET_NUMBER, 10_000_001);

    assert!(Nets::is_normal_net_number(1));
    assert!(Nets::is_normal_net_number(Nets::MAX_LEGAL_NET_NUMBER));
    assert!(!Nets::is_normal_net_number(0));
    assert!(!Nets::is_normal_net_number(-1));
    assert!(!Nets::is_normal_net_number(Nets::MAX_LEGAL_NET_NUMBER + 1));
    assert!(!Nets::is_normal_net_number(Nets::HIDDEN_NET_NUMBER));
}

#[test]
fn via_rule_empty() {
    let empty = ViaRule::empty();
    assert_eq!(empty.name, "empty");
    assert_eq!(empty.via_count(), 0);
    assert!(!empty.contains(&ViaInfo::new("v", PadstackId(0), 1, false)));
    assert_eq!(empty.to_string(), "empty");
}
