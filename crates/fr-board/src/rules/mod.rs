//! Design rules: the clearance matrix, nets, net classes, via definitions and rules, and the
//! `BoardRules` aggregate that owns them all.
//!
//! Java: `rules/{ClearanceMatrix,Net,Nets,NetClass,NetClasses,DefaultItemClearanceClasses,
//! ViaInfo,ViaInfos,ViaRule,BoardRules}.java`.
//!
//! # Design rules for this port (Plan 2)
//!
//! * **No object references between rules.** Java's rules objects point at each other
//!   (`Net.netClass` is a `NetClass`, `ViaRule.list` holds `ViaInfo`s, `ViaInfo.padstack` is a
//!   `Padstack`) and back at the board (`Nets.board`, `NetClass.clearanceMatrix`,
//!   `ViaInfo.boardRules`). Here every such edge is an index newtype from [`crate::ids`]:
//!   [`crate::ids::NetClassId`], [`crate::ids::ViaInfoId`], [`crate::ids::ViaRuleId`],
//!   [`crate::ids::PadstackId`].
//! * **No board back-pointer.** `Nets.board` (Nets.java:24) exists only so `Net` can walk the
//!   board's item list; those `Net` methods become `Board` methods in Task 11 (each is marked
//!   `// not ported:` here, naming its Task 11 replacement).
//! * **No `printInfo`.** Every rules class implements `ItemInfoPrinter.Printable`; the only
//!   call sites are `gui/windows/**` and `drc/ClearanceViolation.printInfo`, which is itself
//!   only called from `gui/windows/routing/WindowClearanceViolations.java:137`. Dropped.
//! * **No `Serializable`.** Java's `BoardRules.writeObject`/`readObject` (BoardRules.java:424-434)
//!   exist purely to round-trip the `transient` `traceAngleRestriction` through Java
//!   serialization; the port has no Java-serialized state.

pub mod board_rules;
pub mod clearance_matrix;
pub mod net;
pub mod net_class;
pub mod via;

pub use board_rules::BoardRules;
pub use clearance_matrix::ClearanceMatrix;
pub use net::{Net, Nets};
pub use net_class::{DefaultItemClearanceClasses, ItemClass, NetClass, NetClasses};
pub use via::{ViaInfo, ViaInfos, ViaRule};

use crate::ids::PadstackId;

/// The padstack facts the rules layer needs from the board library.
///
/// Java's rules classes call straight into `core.library.Padstack`
/// (`ViaRule.getLayerRange`, `BoardRules.createDefaultViaRule`,
/// `BoardRules.getDefaultViaDiameter`). The library module is Task 4's, and it must be free to
/// own its own `Padstacks` type, so the three facts those methods need are expressed as a trait
/// that `Padstacks` will implement. The method names carry a `padstack_` prefix so that Task 4's
/// audit of `core/library` still demands a real `Padstack::from_layer`/`to_layer` rather than
/// being satisfied by this trait.
pub trait PadstackLookup {
    /// `Padstack.fromLayer()` (Padstack.java:137-143): the first layer with a non-null shape.
    /// Java returns `shapes.length` when every shape is null, so this is deliberately a signed
    /// out-of-range-capable layer number rather than a `usize`.
    fn padstack_from_layer(&self, padstack: PadstackId) -> i32;

    /// `Padstack.toLayer()` (Padstack.java:146-152): the last layer with a non-null shape, or
    /// `-1` when every shape is null.
    fn padstack_to_layer(&self, padstack: PadstackId) -> i32;

    /// `Padstack.getShape(layer).maxWidth()`. `None` where Java's `getShape` returns null
    /// (layer out of range, Padstack.java:128-135) or the layer simply has no shape — in Java
    /// both cases are a `NullPointerException` at the `.maxWidth()` call, so callers here
    /// reproduce that as a panic.
    fn padstack_shape_max_width(&self, padstack: PadstackId, layer: i32) -> Option<f64>;
}

/// The one board-item field [`BoardRules::change_clearance_class_index`] and
/// [`BoardRules::remove_clearance_class`] touch.
///
/// Java passes those two methods a `Collection<Item>` (BoardRules.java:263,295) and only ever
/// calls `Item.clearanceClassIndex()` / `Item.setClearanceClassIndex(int)` on its members. Items
/// are Task 5's, so the port names just that pair; `Item` implements this trait once it exists.
pub trait ClearanceClassIndexed {
    /// `Item.clearanceClassIndex()`.
    fn clearance_class_index(&self) -> usize;
    /// `Item.setClearanceClassIndex(int)`.
    fn set_clearance_class_index(&mut self, index: usize);
}

/// Port of `String.equalsIgnoreCase` (used by `ClearanceMatrix.getNo` (ClearanceMatrix.java:60)
/// and `Nets.get` (Nets.java:44,57)).
///
/// Java folds case per *character*: equal, else equal after `Character.toUpperCase`, else equal
/// after lower-casing both upper-cased characters. `str::eq_ignore_ascii_case` would fold only
/// ASCII, which differs for e.g. 'Ä'/'ä'. Rust's `char::to_uppercase` can expand to several
/// characters (ß → SS) where Java's `char`-to-`char` mapping leaves the character alone, so a
/// multi-character expansion is treated as "no mapping", exactly as Java does.
///
/// Divergence: Java folds UTF-16 code units, this folds Unicode scalar values. They differ only
/// for supplementary-plane characters, which cannot appear in a DSN identifier.
pub(crate) fn equals_ignore_case(a: &str, b: &str) -> bool {
    let mut ca = a.chars();
    let mut cb = b.chars();
    loop {
        match (ca.next(), cb.next()) {
            (None, None) => return true,
            (Some(x), Some(y)) => {
                if !chars_equal_ignore_case(x, y) {
                    return false;
                }
            }
            _ => return false,
        }
    }
}

/// Port of `String.CASE_INSENSITIVE_ORDER` as used by `String.compareToIgnoreCase`
/// (`Net.compareTo`, Net.java:61).
///
/// Java's comparator compares character by character; on the first mismatch it retries with both
/// characters upper-cased and then with both of those lower-cased, and returns `c1 - c2` of the
/// lower-cased pair. If one string is a prefix of the other it returns the length difference.
pub(crate) fn compare_to_ignore_case(a: &str, b: &str) -> std::cmp::Ordering {
    let mut ca = a.chars();
    let mut cb = b.chars();
    loop {
        match (ca.next(), cb.next()) {
            (Some(x), Some(y)) => {
                if x == y {
                    continue;
                }
                let (ux, uy) = (java_to_upper(x), java_to_upper(y));
                if ux == uy {
                    continue;
                }
                let (lx, ly) = (java_to_lower(ux), java_to_lower(uy));
                if lx != ly {
                    return (lx as u32).cmp(&(ly as u32));
                }
            }
            // Java compares `length1 - length2` in *characters*; `chars().count()` is the
            // faithful measure here for the same reason `equals_ignore_case` iterates chars.
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
        }
    }
}

/// One step of Java's `String.regionMatches(true, ...)` character comparison.
fn chars_equal_ignore_case(x: char, y: char) -> bool {
    if x == y {
        return true;
    }
    let (ux, uy) = (java_to_upper(x), java_to_upper(y));
    ux == uy || java_to_lower(ux) == java_to_lower(uy)
}

/// `Character.toUpperCase(char)`: a single-character mapping, so a character whose upper case
/// needs more than one character (ß → SS) is returned unchanged.
fn java_to_upper(c: char) -> char {
    let mut it = c.to_uppercase();
    match (it.next(), it.next()) {
        (Some(u), None) => u,
        _ => c,
    }
}

/// `Character.toLowerCase(char)`: as [`java_to_upper`], but lower-casing.
fn java_to_lower(c: char) -> char {
    let mut it = c.to_lowercase();
    match (it.next(), it.next()) {
        (Some(l), None) => l,
        _ => c,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    #[test]
    fn equals_ignore_case_matches_java() {
        assert!(equals_ignore_case("default", "DEFAULT"));
        assert!(equals_ignore_case("Net#1", "net#1"));
        assert!(!equals_ignore_case("default", "defaults"));
        assert!(!equals_ignore_case("a", "b"));
        // Java folds beyond ASCII: 'Ä'.equalsIgnoreCase('ä') is true.
        assert!(equals_ignore_case("\u{c4}", "\u{e4}"));
    }

    #[test]
    fn compare_to_ignore_case_matches_java() {
        assert_eq!(compare_to_ignore_case("abc", "ABC"), Ordering::Equal);
        assert_eq!(compare_to_ignore_case("abc", "abd"), Ordering::Less);
        assert_eq!(compare_to_ignore_case("abd", "ABC"), Ordering::Greater);
        // Prefix: Java returns length1 - length2.
        assert_eq!(compare_to_ignore_case("ab", "abc"), Ordering::Less);
        assert_eq!(compare_to_ignore_case("abc", "ab"), Ordering::Greater);
        assert_eq!(compare_to_ignore_case("", ""), Ordering::Equal);
    }

    #[test]
    fn single_char_case_mapping_leaves_multi_char_expansions_alone() {
        // Character.toUpperCase('ß') == 'ß' in Java (uppercase "SS" does not fit in a
        // char), so 'ß' and "SS" must not fold together.
        assert_eq!(java_to_upper('\u{df}'), '\u{df}');
        assert!(!equals_ignore_case("\u{df}", "SS"));
    }
}
