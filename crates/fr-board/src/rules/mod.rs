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
pub use clearance_matrix::{CLEARANCE_SAFETY_MARGIN, ClearanceMatrix};
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
/// Java folds case per UTF-16 *code unit*: equal, else equal after `Character.toUpperCase`,
/// else equal after lower-casing both upper-cased units. `str::eq_ignore_ascii_case` would fold
/// only ASCII, which differs for e.g. 'Ä'/'ä', so this reproduces Java's three-step fold over
/// `encode_utf16()` — the same domain Java works in, which makes the port exact for
/// supplementary-plane input too (Java case-folds each surrogate half, i.e. not at all, and so
/// does [`java_to_upper_unit`]).
pub(crate) fn equals_ignore_case(a: &str, b: &str) -> bool {
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

/// Port of `String.CASE_INSENSITIVE_ORDER` as used by `String.compareToIgnoreCase`
/// (`Net.compareTo`, Net.java:61).
///
/// Java's comparator compares code unit by code unit; on the first mismatch it retries with both
/// units upper-cased and then with both of those lower-cased, and returns `c1 - c2` of the
/// lower-cased pair (an unsigned 16-bit difference). If one string is a prefix of the other it
/// returns the code-unit length difference. Only the sign is observable through `Ordering`.
pub(crate) fn compare_to_ignore_case(a: &str, b: &str) -> std::cmp::Ordering {
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

/// One step of Java's `String.regionMatches(true, ...)` code-unit comparison.
fn units_equal_ignore_case(x: u16, y: u16) -> bool {
    if x == y {
        return true;
    }
    let (ux, uy) = (java_to_upper_unit(x), java_to_upper_unit(y));
    ux == uy || java_to_lower_unit(ux) == java_to_lower_unit(uy)
}

/// `Character.toUpperCase(char)` on a UTF-16 code unit. An unpaired surrogate has no case
/// mapping in Java either, so it is returned unchanged.
fn java_to_upper_unit(u: u16) -> u16 {
    map_unit(u, java_to_upper)
}

/// `Character.toLowerCase(char)` on a UTF-16 code unit.
fn java_to_lower_unit(u: u16) -> u16 {
    map_unit(u, java_to_lower)
}

fn map_unit(u: u16, f: fn(char) -> char) -> u16 {
    match char::from_u32(u32::from(u)) {
        // Java returns the original `char` whenever the mapping does not fit in one `char`.
        Some(c) => u16::try_from(f(c) as u32).unwrap_or(u),
        None => u, // a lone surrogate
    }
}

/// `Character.toUpperCase(char)`: Java's **simple** (single-character) case mapping.
///
/// Rust's `char::to_uppercase` is the **full** mapping, which expands where Java's cannot: 'ß'
/// upper-cases to "SS" in Rust and stays 'ß' in Java, so a multi-character expansion normally
/// means "Java leaves this alone". The exception is
/// [`java_simple_uppercase_exception`] — see there.
fn java_to_upper(c: char) -> char {
    if let Some(u) = java_simple_uppercase_exception(c) {
        return u;
    }
    let mut it = c.to_uppercase();
    match (it.next(), it.next()) {
        (Some(u), None) => u,
        _ => c,
    }
}

/// `Character.toLowerCase(char)`: as [`java_to_upper`], but lower-casing.
///
/// U+0130 (LATIN CAPITAL LETTER I WITH DOT ABOVE) is the *only* BMP character whose full
/// lowercase expands ("i" + U+0307 COMBINING DOT ABOVE) while Java's simple lowercase is a
/// single character, `'i'`. Without this arm `"İ".equalsIgnoreCase("ı")` would answer false
/// where Java answers true.
fn java_to_lower(c: char) -> char {
    if c == '\u{130}' {
        return 'i';
    }
    let mut it = c.to_lowercase();
    match (it.next(), it.next()) {
        (Some(l), None) => l,
        _ => c,
    }
}

/// The characters whose *full* uppercase expands to several characters while Java's *simple*
/// uppercase is a real single character — the mirror of `java_to_lower`'s U+0130 arm.
///
/// These 27 Greek letters with ypogegrammeni have a full uppercase of "base + U+0399" but a
/// simple uppercase that is the corresponding capital-with-prosgegrammeni letter, e.g.
/// `Character.toUpperCase('\u{1F80}') == '\u{1F88}'`.
///
/// The list is complete, not a guess: all 65 536 BMP code points were swept against JDK 23's
/// `Character.toUpperCase(char)` / `toLowerCase(char)` (harness in the Task 2 fix report), and
/// these 27 plus U+0130 are the only differences that are *not* Unicode-version skew. The skew
/// cases — U+019B, U+0264, U+1C89, U+1C8A, U+A7CB, U+A7CC, U+A7CD, U+A7CE, U+A7CF, U+A7D2,
/// U+A7D3, U+A7D4, U+A7D5, U+A7DA, U+A7DB, U+A7DC — are characters whose mappings exist in the
/// Unicode version Rust's `std` tracks but not in JDK 23's, or vice versa. No mapping rule can
/// reconcile those without freezing a UCD table, and none of them can appear in a DSN
/// identifier, a net name, or a clearance-class name.
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

    // Every expectation below was produced by running `String.equalsIgnoreCase` /
    // `String.compareToIgnoreCase` on the same inputs under JDK 23 (harness in the Task 2 fix
    // report). The whole BMP was additionally swept against `Character.toUpperCase(char)` /
    // `toLowerCase(char)`, and 82,350 string pairs against the two `String` methods, with zero
    // mismatches.

    /// LATIN CAPITAL LETTER I WITH DOT ABOVE.
    const I_DOT: &str = "\u{130}";
    /// LATIN SMALL LETTER DOTLESS I.
    const DOTLESS_I: &str = "\u{131}";
    /// KELVIN SIGN (not the letter K).
    const KELVIN: &str = "\u{212a}";
    /// OHM SIGN (not the Greek capital omega).
    const OHM: &str = "\u{2126}";
    /// LATIN SMALL LIGATURE FF, whose full uppercase is "FF".
    const LIG_FF: &str = "\u{fb00}";
    /// GREEK SMALL LETTER ALPHA WITH PSILI AND YPOGEGRAMMENI and its simple-uppercase partner.
    const YPO: &str = "\u{1f80}";
    const YPO_CAPITAL: &str = "\u{1f88}";

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
    fn equals_ignore_case_folds_the_dotted_and_dotless_i_like_java() {
        // `Character.toLowerCase('\u{130}') == 'i'` in Java (a *simple* mapping); Rust's full
        // mapping expands to "i" + U+0307, which is why `java_to_lower` special-cases it.
        assert!(equals_ignore_case(I_DOT, DOTLESS_I));
        assert!(equals_ignore_case(I_DOT, "i"));
        assert!(equals_ignore_case(I_DOT, "I"));
        assert!(equals_ignore_case(DOTLESS_I, "I"));
        assert!(equals_ignore_case(DOTLESS_I, "i"));
    }

    #[test]
    fn equals_ignore_case_does_not_expand_multi_character_case_mappings() {
        // Java's char-to-char mapping leaves these alone, so they never fold to their
        // multi-character upper case.
        assert!(!equals_ignore_case("\u{df}", "SS"));
        assert!(!equals_ignore_case("\u{df}", "ss"));
        assert!(!equals_ignore_case(LIG_FF, "FF"));
        assert!(equals_ignore_case(LIG_FF, LIG_FF));
        // But 'ß' does fold against U+1E9E, whose *simple* lowercase is 'ß'.
        assert!(equals_ignore_case("\u{df}", "\u{1e9e}"));
    }

    #[test]
    fn equals_ignore_case_folds_the_compatibility_symbols_like_java() {
        assert!(equals_ignore_case(KELVIN, "k"));
        assert!(equals_ignore_case(KELVIN, "K"));
        assert!(!equals_ignore_case(KELVIN, "L"));
        assert!(equals_ignore_case(OHM, "\u{3c9}"));
        // The ypogegrammeni letters, the mirror of the U+0130 case on the uppercase side.
        assert!(equals_ignore_case(YPO, YPO_CAPITAL));
        assert!(equals_ignore_case("\u{1fb3}", "\u{1fbc}"));
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
    fn compare_to_ignore_case_orders_the_special_case_mappings_like_java() {
        // The pair the U+0130 fix changes: Java lower-cases it to 'i' (U+0069), which sorts
        // *before* 'l' (U+006C); without the fix it stayed U+0130 and sorted after.
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
        // Character.toUpperCase / toLowerCase under JDK 23, spot values from the BMP sweep.
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
        // The complete ypogegrammeni exception set: +8 for the three runs of eight, +9 for the
        // three singletons.
        assert_eq!(java_to_upper('\u{1f80}'), '\u{1f88}');
        assert_eq!(java_to_upper('\u{1f87}'), '\u{1f8f}');
        assert_eq!(java_to_upper('\u{1f90}'), '\u{1f98}');
        assert_eq!(java_to_upper('\u{1fa7}'), '\u{1faf}');
        assert_eq!(java_to_upper('\u{1fb3}'), '\u{1fbc}');
        assert_eq!(java_to_upper('\u{1fc3}'), '\u{1fcc}');
        assert_eq!(java_to_upper('\u{1ff3}'), '\u{1ffc}');
        // Not in the exception set: the neighbouring code points keep the ordinary rule.
        assert_eq!(java_to_upper('\u{1f88}'), '\u{1f88}');
        assert_eq!(java_to_upper('\u{1fb2}'), '\u{1fb2}');
    }

    #[test]
    fn folding_works_on_utf16_code_units_like_java() {
        // A supplementary character is two surrogate code units in Java, neither of which has a
        // case mapping, so DESERET CAPITAL LETTER LONG I does *not* fold to its small form.
        let capital = "\u{10400}";
        let small = "\u{10428}";
        assert!(!equals_ignore_case(capital, small));
        assert!(equals_ignore_case(capital, capital));
        // Length is compared in code units: one supplementary character is longer than one BMP
        // character.
        assert_eq!(compare_to_ignore_case(capital, "a"), Ordering::Greater);
    }
}
