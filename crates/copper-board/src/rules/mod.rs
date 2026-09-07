pub mod board_rules;
pub mod clearance_matrix;
pub mod drc_constraints;
pub mod net;
pub mod net_class;
pub mod via;

pub use board_rules::BoardRules;
pub use clearance_matrix::{CLEARANCE_SAFETY_MARGIN, ClearanceMatrix};
pub use drc_constraints::{DrcConstraints, DrcSeverity};
pub use net::{Net, Nets};
pub use net_class::{DefaultItemClearanceClasses, ItemClass, NetClass, NetClasses};
pub use via::{ViaInfo, ViaInfos, ViaRule};

use crate::ids::PadstackId;

pub trait PadstackLookup {
    fn padstack_from_layer(&self, padstack: PadstackId) -> i32;

    fn padstack_to_layer(&self, padstack: PadstackId) -> i32;

    fn padstack_shape_max_width(&self, padstack: PadstackId, layer: i32) -> Option<f64>;
}

pub trait ClearanceClassIndexed {
    fn clearance_class_index(&self) -> usize;
    fn set_clearance_class_index(&mut self, index: usize);
}

pub fn equals_ignore_case(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

pub fn compare_to_ignore_case(a: &str, b: &str) -> std::cmp::Ordering {
    a.chars()
        .map(|c| c.to_ascii_lowercase())
        .cmp(b.chars().map(|c| c.to_ascii_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    #[test]
    fn equals_ignore_case_is_ascii_case_insensitive() {
        assert!(equals_ignore_case("default", "DEFAULT"));
        assert!(equals_ignore_case("Net#1", "net#1"));
        assert!(!equals_ignore_case("default", "defaults"));
        assert!(!equals_ignore_case("a", "b"));
        assert!(equals_ignore_case("", ""));
    }

    #[test]
    fn compare_to_ignore_case_orders_ascii_case_insensitively() {
        assert_eq!(compare_to_ignore_case("abc", "ABC"), Ordering::Equal);
        assert_eq!(compare_to_ignore_case("abc", "abd"), Ordering::Less);
        assert_eq!(compare_to_ignore_case("abd", "ABC"), Ordering::Greater);
        assert_eq!(compare_to_ignore_case("ab", "abc"), Ordering::Less);
        assert_eq!(compare_to_ignore_case("abc", "ab"), Ordering::Greater);
        assert_eq!(compare_to_ignore_case("", ""), Ordering::Equal);
    }
}
