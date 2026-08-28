//! Port of `src/test/java/app/freerouting/rules/ClearanceMatrixTest.java`, plus the extra
//! clearance-matrix / nets / via-rule cases the Task 2 brief asks for.
//!
//! Every expected value is derived from the Java source, with the line cited.

use fr_board::ids::ViaInfoId;
use fr_board::rules::clearance_matrix::CLEARANCE_SAFETY_MARGIN;
use fr_board::rules::{ClearanceMatrix, Nets, ViaRule};
use fr_board::structure::{Layer, LayerStructure};

/// The `Layer[] {Top, Bottom}` stack of `ClearanceMatrixTest.java:13`.
fn two_layer_structure() -> LayerStructure {
    LayerStructure::new(vec![Layer::new("Top", true), Layer::new("Bottom", true)])
}

/// Port of `ClearanceMatrixTest.setValue` (ClearanceMatrixTest.java:12-36).
#[test]
fn set_value_matches_java_test() {
    let layer_structure = two_layer_structure();
    let mut matrix = ClearanceMatrix::new(1, &layer_structure, &["default"]);

    // Test with an odd value: `setValue` rounds odd values up (ClearanceMatrix.java:107-112).
    matrix.set_value(0, 0, 0, 5);
    assert_eq!(matrix.get_value(0, 0, 0, false), 6);
    assert_eq!(matrix.max_value(0, 0), 6);
    assert_eq!(matrix.max_value_on_layer(0), 6);

    // Test with a negative value: clamped to 0 (ClearanceMatrix.java:106).
    matrix.set_value(0, 0, 0, -10);
    assert_eq!(matrix.get_value(0, 0, 0, false), 0);
    // The maxima are running maxima (ClearanceMatrix.java:116-117), so they stay at 6.
    assert_eq!(matrix.max_value(0, 0), 6);
    assert_eq!(matrix.max_value_on_layer(0), 6);

    // Test with Integer.MAX_VALUE: odd, so it is rounded *down* (ClearanceMatrix.java:108-109).
    matrix.set_value(0, 0, 0, i32::MAX);
    assert_eq!(matrix.get_value(0, 0, 0, false), i32::MAX - 1);
    assert_eq!(matrix.max_value(0, 0), i32::MAX - 1);
    assert_eq!(matrix.max_value_on_layer(0), i32::MAX - 1);
}

/// `getValue` reads `row[classJ].column[classI]` (ClearanceMatrix.java:163) and `setValue`
/// writes the same entry (ClearanceMatrix.java:101-102) — the second argument picks the row.
/// `setValue` is *not* symmetric: it writes exactly one entry, so the transposed read still
/// answers the untouched 0.
#[test]
fn get_value_uses_j_then_i_indexing() {
    let layer_structure = two_layer_structure();
    let mut matrix = ClearanceMatrix::new(3, &layer_structure, &["null", "default", "power"]);

    matrix.set_value(1, 2, 0, 100);
    assert_eq!(matrix.get_value(1, 2, 0, false), 100);
    // Java writes only `row[2].column[1]`; `row[1].column[2]` was never touched.
    assert_eq!(matrix.get_value(2, 1, 0, false), 0);
    // Layer 1 of the same entry was not written either.
    assert_eq!(matrix.get_value(1, 2, 1, false), 0);

    // The row maximum is kept on the J axis (ClearanceMatrix.java:116), so class 2 sees 100 and
    // class 1 does not.
    assert_eq!(matrix.max_value(2, 0), 100);
    assert_eq!(matrix.max_value(1, 0), 0);
}

/// `clearance_safety_margin = 16` (ClearanceMatrix.java:17) is added to the stored value when
/// `addSafetyMargin` is true (ClearanceMatrix.java:164-165), and to nothing else — the
/// out-of-bounds early return at :160 returns a bare 0.
#[test]
fn safety_margin_adds_16() {
    assert_eq!(CLEARANCE_SAFETY_MARGIN, 16);

    let layer_structure = two_layer_structure();
    let mut matrix = ClearanceMatrix::new(2, &layer_structure, &["null", "default"]);
    matrix.set_value(1, 1, 0, 40);

    assert_eq!(matrix.get_value(1, 1, 0, false), 40);
    assert_eq!(matrix.get_value(1, 1, 0, true), 56);
    // An untouched entry is 0 + 16.
    assert_eq!(matrix.get_value(0, 0, 0, true), 16);
    // Out of bounds returns 0 with no margin (ClearanceMatrix.java:133-160).
    assert_eq!(matrix.get_value(2, 1, 0, true), 0);
    assert_eq!(matrix.get_value(1, 1, 2, true), 0);
}

/// `Nets.max_legal_net_number = 9999999` (Nets.java:16) and
/// `Nets.hidden_net_number = 10000001` (Nets.java:19); `isNormalNetNumber` is
/// `netNumber > 0 && netNumber <= max_legal_net_number` (Nets.java:33).
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

/// `ViaRule.EMPTY = new ViaRule("empty")` (ViaRule.java:18): named "empty", no vias.
#[test]
fn via_rule_empty() {
    let empty = ViaRule::empty();
    assert_eq!(empty.name, "empty");
    assert_eq!(empty.via_count(), 0);
    assert!(!empty.contains(ViaInfoId(0)));
    assert_eq!(empty.to_string(), "empty");
}
