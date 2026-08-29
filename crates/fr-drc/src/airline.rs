//! `drc.AirLine`: one incomplete connection of one net — the ratsnest line the GUI draws and the
//! DRC's `incompleteCount` counts.
//!
//! Java: `drc/AirLine.java` (55 lines, five final fields, a constructor, `compareTo` and
//! `toString`). [`crate::net_incompletes::NetIncompletes`] is its only producer.

use std::cmp::Ordering;

use fr_board::{ItemId, Nets};
use fr_geometry::FloatPoint;

/// Port of `drc.AirLine` (AirLine.java:12-55).
///
/// Java holds the `Net` object (`:15`); the port holds its **number** and looks the net up in
/// `board.rules.nets` when it needs the name, per `global-constraints.md`'s no-back-pointer rule
/// — the same choice [`crate::UnconnectedItems`] makes for its items.
///
// not ported: AirLine.toString (AirLine.java:51-54) — `net.name + ": " + fromItem + " - " +
// toItem`, i.e. two `Item.toString()`s, which are `TextManager`-localised GUI strings. Nothing
// headless reads it; `generateReport` builds its own descriptions.
#[derive(Debug, Clone, PartialEq)]
pub struct AirLine {
    /// Java `net` (AirLine.java:15), as a net number.
    pub net_number: i32,
    /// Java `fromItem` (AirLine.java:18): the item the airline starts at.
    pub from_item: ItemId,
    /// Java `fromCorner` (AirLine.java:21): the exact start coordinate.
    pub from_corner: FloatPoint,
    /// Java `toItem` (AirLine.java:24): the item the airline ends at.
    pub to_item: ItemId,
    /// Java `toCorner` (AirLine.java:27): the exact end coordinate.
    pub to_corner: FloatPoint,
}

impl AirLine {
    /// Port of `AirLine(Net, Item, FloatPoint, Item, FloatPoint)` (AirLine.java:38-44).
    pub fn new(
        net_number: i32,
        from_item: ItemId,
        from_corner: FloatPoint,
        to_item: ItemId,
        to_corner: FloatPoint,
    ) -> AirLine {
        AirLine {
            net_number,
            from_item,
            from_corner,
            to_item,
            to_corner,
        }
    }

    /// Port of `AirLine.compareTo` (AirLine.java:46-49): `this.net.name.compareTo(other.net.name)`
    /// and nothing else.
    ///
    // renamed: AirLine.compareTo -> compare_by_net_name, and `Ord` is deliberately **not**
    // derived or implemented. The Java comparator is not antisymmetric — two airlines of the same
    // net compare `0` however different their endpoints — so a `TreeSet<AirLine>` would collapse
    // every airline of one net into a single element and a `sort` would be unstable in the
    // mathematical sense. No caller in the Java tree does either (`getAllAirlines` returns an
    // array in `LinkedList` order, DesignRulesChecker.java:780-798), so the bug is latent; the
    // port keeps the comparator reachable under a name that says what it compares, following
    // quirk #1's precedent for Java's other non-antisymmetric comparators. Quirks row #148.
    ///
    /// `nets` resolves the two net numbers to names; a number with no net reads as the empty
    /// string, so it sorts before every present name and equal to another absent one.
    ///
    // totalized: AirLine.compareTo dereferences `this.net.name` (AirLine.java:48) and throws a
    // `NullPointerException` when the net is absent. Unreachable from the port's own producer:
    // `NetIncompletes` NPEs at NetIncompletes.java:300 long before it builds an airline for a net
    // number `board.rules.nets.get` does not know. Treating the missing name as `""` keeps the
    // function total.
    pub fn compare_by_net_name(&self, other: &AirLine, nets: &Nets) -> Ordering {
        let name = |number: i32| nets.get(number).map(|net| net.name.as_str()).unwrap_or("");
        java_string_compare(name(self.net_number), name(other.net_number))
    }
}

/// `java.lang.String.compareTo`: lexicographic over **UTF-16 code units**, then by length.
///
/// Rust's `str::cmp` compares UTF-8 bytes, which agrees with Java on every string whose
/// characters are all below U+10000 but disagrees once a surrogate pair meets a code point in
/// U+E000..U+FFFF. `fr_board::compare_to_ignore_case` (rules/mod.rs:110-133) already sets the
/// precedent of encoding to UTF-16 first; this is the case-sensitive sibling.
fn java_string_compare(a: &str, b: &str) -> Ordering {
    let mut ca = a.encode_utf16();
    let mut cb = b.encode_utf16();
    loop {
        match (ca.next(), cb.next()) {
            (Some(x), Some(y)) if x == y => continue,
            (Some(x), Some(y)) => return x.cmp(&y),
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_string_compare_is_utf16_code_unit_order() {
        // The one case where Java and `str::cmp` disagree: U+FFFD (one UTF-16 unit, 0xFFFD)
        // against U+10000 (the surrogate pair D800 DC00). Java says U+10000 < U+FFFD; UTF-8 byte
        // order says the opposite.
        assert_eq!(java_string_compare("\u{10000}", "\u{FFFD}"), Ordering::Less);
        assert_eq!("\u{10000}".cmp("\u{FFFD}"), Ordering::Greater);

        assert_eq!(java_string_compare("GND", "GND"), Ordering::Equal);
        assert_eq!(java_string_compare("GND", "VCC"), Ordering::Less);
        assert_eq!(java_string_compare("GND", "GN"), Ordering::Greater);
    }
}
