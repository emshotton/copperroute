use std::cmp::Ordering;

use fr_board::{ItemId, Nets};
use fr_geometry::FloatPoint;

#[derive(Debug, Clone, PartialEq)]
pub struct AirLine {
        pub net_number: i32,
        pub from_item: ItemId,
        pub from_corner: FloatPoint,
        pub to_item: ItemId,
        pub to_corner: FloatPoint,
}

impl AirLine {
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

                                pub fn compare_by_net_name(&self, other: &AirLine, nets: &Nets) -> Ordering {
        let name = |number: i32| nets.get(number).map(|net| net.name.as_str()).unwrap_or("");
        java_string_compare(name(self.net_number), name(other.net_number))
    }
}

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
        assert_eq!(java_string_compare("\u{10000}", "\u{FFFD}"), Ordering::Less);
        assert_eq!("\u{10000}".cmp("\u{FFFD}"), Ordering::Greater);

        assert_eq!(java_string_compare("GND", "GND"), Ordering::Equal);
        assert_eq!(java_string_compare("GND", "VCC"), Ordering::Less);
        assert_eq!(java_string_compare("GND", "GN"), Ordering::Greater);
    }
}
