pub mod board_rules;
pub mod clearance_matrix;
pub mod net;
pub mod net_class;
pub mod via;

pub use board_rules::BoardRules;
pub use clearance_matrix::{CLEARANCE_SAFETY_MARGIN, ClearanceMatrix};
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
    let mut ca = a.encode_utf16();
    let mut cb = b.encode_utf16();
    loop {
        match (ca.next(), cb.next()) {
            (None, None) => return true,
            (Some(x), Some(y)) => {
                if !units_equal_ignore_case(x, y) {
                    return false;
                }
            }
            _ => return false,
        }
    }
}

pub fn compare_to_ignore_case(a: &str, b: &str) -> std::cmp::Ordering {
    let mut ca = a.encode_utf16();
    let mut cb = b.encode_utf16();
    loop {
        match (ca.next(), cb.next()) {
            (Some(x), Some(y)) => {
                if x == y {
                    continue;
                }
                let (ux, uy) = (java_to_upper_unit(x), java_to_upper_unit(y));
                if ux == uy {
                    continue;
                }
                let (lx, ly) = (java_to_lower_unit(ux), java_to_lower_unit(uy));
                if lx != ly {
                    return lx.cmp(&ly);
                }
            }
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
        }
    }
}

fn units_equal_ignore_case(x: u16, y: u16) -> bool {
    if x == y {
        return true;
    }
    let (ux, uy) = (java_to_upper_unit(x), java_to_upper_unit(y));
    ux == uy || java_to_lower_unit(ux) == java_to_lower_unit(uy)
}

fn java_to_upper_unit(u: u16) -> u16 {
    map_unit(u, java_to_upper)
}

fn java_to_lower_unit(u: u16) -> u16 {
    map_unit(u, java_to_lower)
}

fn map_unit(u: u16, f: fn(char) -> char) -> u16 {
    match char::from_u32(u32::from(u)) {
        Some(c) => u16::try_from(f(c) as u32).unwrap_or(u),
        None => u,
    }
}

pub fn java_to_upper(c: char) -> char {
    if let Some(u) = java_simple_uppercase_exception(c) {
        return u;
    }
    let mut it = c.to_uppercase();
    match (it.next(), it.next()) {
        (Some(u), None) => u,
        _ => c,
    }
}

pub fn java_to_lower(c: char) -> char {
    if c == '\u{130}' {
        return 'i';
    }
    let mut it = c.to_lowercase();
    match (it.next(), it.next()) {
        (Some(l), None) => l,
        _ => c,
    }
}

fn java_simple_uppercase_exception(c: char) -> Option<char> {
    let mapped = match c as u32 {
        0x1F80..=0x1F87 | 0x1F90..=0x1F97 | 0x1FA0..=0x1FA7 => c as u32 + 8,
        0x1FB3 | 0x1FC3 | 0x1FF3 => c as u32 + 9,
        _ => return None,
    };
    char::from_u32(mapped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    const I_DOT: &str = "\u{130}";
    const DOTLESS_I: &str = "\u{131}";
    const KELVIN: &str = "\u{212a}";
    const OHM: &str = "\u{2126}";
    const LIG_FF: &str = "\u{fb00}";
    const YPO: &str = "\u{1f80}";
    const YPO_CAPITAL: &str = "\u{1f88}";

    #[test]
    fn equals_ignore_case_matches_java() {
        assert!(equals_ignore_case("default", "DEFAULT"));
        assert!(equals_ignore_case("Net#1", "net#1"));
        assert!(!equals_ignore_case("default", "defaults"));
        assert!(!equals_ignore_case("a", "b"));
        assert!(equals_ignore_case("\u{c4}", "\u{e4}"));
    }

    #[test]
    fn equals_ignore_case_folds_the_dotted_and_dotless_i_like_java() {
        assert!(equals_ignore_case(I_DOT, DOTLESS_I));
        assert!(equals_ignore_case(I_DOT, "i"));
        assert!(equals_ignore_case(I_DOT, "I"));
        assert!(equals_ignore_case(DOTLESS_I, "I"));
        assert!(equals_ignore_case(DOTLESS_I, "i"));
    }

    #[test]
    fn equals_ignore_case_does_not_expand_multi_character_case_mappings() {
        assert!(!equals_ignore_case("\u{df}", "SS"));
        assert!(!equals_ignore_case("\u{df}", "ss"));
        assert!(!equals_ignore_case(LIG_FF, "FF"));
        assert!(equals_ignore_case(LIG_FF, LIG_FF));
        assert!(equals_ignore_case("\u{df}", "\u{1e9e}"));
    }

    #[test]
    fn equals_ignore_case_folds_the_compatibility_symbols_like_java() {
        assert!(equals_ignore_case(KELVIN, "k"));
        assert!(equals_ignore_case(KELVIN, "K"));
        assert!(!equals_ignore_case(KELVIN, "L"));
        assert!(equals_ignore_case(OHM, "\u{3c9}"));
        assert!(equals_ignore_case(YPO, YPO_CAPITAL));
        assert!(equals_ignore_case("\u{1fb3}", "\u{1fbc}"));
    }

    #[test]
    fn compare_to_ignore_case_matches_java() {
        assert_eq!(compare_to_ignore_case("abc", "ABC"), Ordering::Equal);
        assert_eq!(compare_to_ignore_case("abc", "abd"), Ordering::Less);
        assert_eq!(compare_to_ignore_case("abd", "ABC"), Ordering::Greater);
        assert_eq!(compare_to_ignore_case("ab", "abc"), Ordering::Less);
        assert_eq!(compare_to_ignore_case("abc", "ab"), Ordering::Greater);
        assert_eq!(compare_to_ignore_case("", ""), Ordering::Equal);
    }

    #[test]
    fn compare_to_ignore_case_orders_the_special_case_mappings_like_java() {
        assert_eq!(compare_to_ignore_case(I_DOT, "L"), Ordering::Less);
        assert_eq!(compare_to_ignore_case("L", I_DOT), Ordering::Greater);
        assert_eq!(compare_to_ignore_case(I_DOT, DOTLESS_I), Ordering::Equal);
        assert_eq!(compare_to_ignore_case(I_DOT, "i"), Ordering::Equal);

        assert_eq!(compare_to_ignore_case(KELVIN, "k"), Ordering::Equal);
        assert_eq!(compare_to_ignore_case(KELVIN, "L"), Ordering::Less);
        assert_eq!(compare_to_ignore_case("\u{df}", "SS"), Ordering::Greater);
        assert_eq!(compare_to_ignore_case(LIG_FF, "FF"), Ordering::Greater);
        assert_eq!(compare_to_ignore_case(YPO, YPO_CAPITAL), Ordering::Equal);
        assert_eq!(compare_to_ignore_case(YPO, "L"), Ordering::Greater);
        assert_eq!(compare_to_ignore_case(YPO_CAPITAL, "L"), Ordering::Greater);
    }

    #[test]
    fn simple_case_mappings_match_java_character_methods() {
        assert_eq!(java_to_lower('\u{130}'), 'i');
        assert_eq!(java_to_upper('\u{130}'), '\u{130}');
        assert_eq!(java_to_upper('\u{131}'), 'I');
        assert_eq!(java_to_upper('\u{df}'), '\u{df}');
        assert_eq!(java_to_lower('\u{df}'), '\u{df}');
        assert_eq!(java_to_upper('\u{212a}'), '\u{212a}');
        assert_eq!(java_to_lower('\u{212a}'), 'k');
        assert_eq!(java_to_upper('\u{2126}'), '\u{2126}');
        assert_eq!(java_to_lower('\u{2126}'), '\u{3c9}');
        assert_eq!(java_to_upper('\u{fb00}'), '\u{fb00}');
        assert_eq!(java_to_upper('\u{1f80}'), '\u{1f88}');
        assert_eq!(java_to_upper('\u{1f87}'), '\u{1f8f}');
        assert_eq!(java_to_upper('\u{1f90}'), '\u{1f98}');
        assert_eq!(java_to_upper('\u{1fa7}'), '\u{1faf}');
        assert_eq!(java_to_upper('\u{1fb3}'), '\u{1fbc}');
        assert_eq!(java_to_upper('\u{1fc3}'), '\u{1fcc}');
        assert_eq!(java_to_upper('\u{1ff3}'), '\u{1ffc}');
        assert_eq!(java_to_upper('\u{1f88}'), '\u{1f88}');
        assert_eq!(java_to_upper('\u{1fb2}'), '\u{1fb2}');
    }

    #[test]
    fn folding_works_on_utf16_code_units_like_java() {
        let capital = "\u{10400}";
        let small = "\u{10428}";
        assert!(!equals_ignore_case(capital, small));
        assert!(equals_ignore_case(capital, capital));
        assert_eq!(compare_to_ignore_case(capital, "a"), Ordering::Greater);
    }
}
