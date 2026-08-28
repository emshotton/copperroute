//! Logical parts: gate-swap and pin-swap information for the Specctra DSN format.
//!
//! Java: `core/library/LogicalPart.java`, `core/library/LogicalParts.java`.

use std::cmp::Ordering;

use crate::rules::equals_ignore_case;

/// Port of the nested `LogicalPart.PartPin` (LogicalPart.java:68-114): a pin belonging to a
/// logical part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartPin {
    /// `PartPin.pinIndex` (LogicalPart.java:71): must be the same index as in the component's
    /// library package.
    pub pin_index: i32,
    /// `PartPin.pinName` (LogicalPart.java:74): must be the same name as in the component's
    /// library package.
    pub pin_name: String,
    /// `PartPin.gateName` (LogicalPart.java:77): the gate this pin belongs to.
    pub gate_name: String,
    /// `PartPin.gateSwapCode` (LogicalPart.java:83): gates with the same code (> 0) can be
    /// swapped; codes `<= 0` are not swappable.
    pub gate_swap_code: i32,
    /// `PartPin.gatePinName` (LogicalPart.java:86): the pin's identifier within the gate.
    pub gate_pin_name: String,
    /// `PartPin.gatePinSwapCode` (LogicalPart.java:92): pins with the same code (> 0) can be
    /// swapped inside a gate; codes `<= 0` are not swappable.
    pub gate_pin_swap_code: i32,
}

impl PartPin {
    /// Port of the `PartPin(int, String, String, int, String, int)` constructor
    /// (LogicalPart.java:95-108).
    pub fn new(
        pin_index: i32,
        pin_name: impl Into<String>,
        gate_name: impl Into<String>,
        gate_swap_code: i32,
        gate_pin_name: impl Into<String>,
        gate_pin_swap_code: i32,
    ) -> PartPin {
        PartPin {
            pin_index,
            pin_name: pin_name.into(),
            gate_name: gate_name.into(),
            gate_swap_code,
            gate_pin_name: gate_pin_name.into(),
            gate_pin_swap_code,
        }
    }

    /// Port of `PartPin.compareTo` (LogicalPart.java:110-113): orders by `pinIndex` alone.
    ///
    /// Java computes `this.pinIndex - other.pinIndex`, which can overflow for extreme `int`
    /// values; the port uses `Ord::cmp` instead, which agrees with Java's subtraction on every
    /// non-overflowing input (the only ones that occur — pin indices are small non-negative
    /// counts) without the overflow hazard. Not an `Ord` impl (see `Padstack::compare_to`):
    /// two pins with the same `pin_index` but different names compare `Equal` here while
    /// `PartialEq` is structural.
    pub fn compare_to(&self, other: &PartPin) -> Ordering {
        self.pin_index.cmp(&other.pin_index)
    }
}

/// Port of `LogicalPart` (`core/library/LogicalPart.java`): information for gate swap and pin
/// swap for a single component.
///
/// not ported: `LogicalPart.printInfo` (LogicalPart.java:40-65) — `ItemInfoPrinter.Printable`,
/// GUI only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicalPart {
    /// `LogicalPart.name` (LogicalPart.java:12).
    pub name: String,
    /// `LogicalPart.id` (LogicalPart.java:13), renamed per the task brief; starts at 1
    /// (`LogicalParts.add`, LogicalParts.java:17).
    pub no: usize,
    /// `LogicalPart.partPinArr` (LogicalPart.java:14), sorted by `pin_index` before storage
    /// (`LogicalParts.add`, LogicalParts.java:16).
    part_pins: Vec<PartPin>,
}

impl LogicalPart {
    /// Port of the `LogicalPart(String, int, PartPin[])` constructor (LogicalPart.java:20-24).
    /// Only [`LogicalParts::add`] calls this, matching Java's public-but-only-caller usage
    /// (Java's constructor is public, but the sort in `LogicalParts.add` is required for the
    /// class doc's invariant, so the port keeps construction private to this module).
    pub(crate) fn new(name: impl Into<String>, no: usize, part_pins: Vec<PartPin>) -> LogicalPart {
        LogicalPart {
            name: name.into(),
            no,
            part_pins,
        }
    }

    /// Port of `LogicalPart.pinCount` (LogicalPart.java:26-29).
    pub fn pin_count(&self) -> usize {
        self.part_pins.len()
    }

    /// Port of `LogicalPart.getPin` (LogicalPart.java:31-38): the pin at `pin_index`, or `None`
    /// when out of range. Java's out-of-range branch also logs a warning (dropped).
    pub fn get_pin(&self, pin_index: i32) -> Option<&PartPin> {
        if pin_index < 0 || pin_index as usize >= self.part_pins.len() {
            return None;
        }
        Some(&self.part_pins[pin_index as usize])
    }
}

/// Port of `LogicalParts` (`core/library/LogicalParts.java`): the database of logical parts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LogicalParts {
    /// `LogicalParts.partArr` (LogicalParts.java:12, a `Vector`).
    list: Vec<LogicalPart>,
}

impl LogicalParts {
    /// An empty logical-part database.
    pub fn new() -> LogicalParts {
        LogicalParts::default()
    }

    /// Port of `LogicalParts.add` (LogicalParts.java:15-20): sorts `part_pins` by `pin_index`
    /// (`Arrays.sort`, using [`PartPin::compare_to`]) and appends a new logical part, returning
    /// its freshly assigned id.
    ///
    /// Rust's `slice::sort_by` is a stable sort, matching Java's `Arrays.sort(Object[])`
    /// (TimSort, also stable), so pins with equal `pin_index` keep their input order.
    pub fn add(&mut self, name: impl Into<String>, mut part_pins: Vec<PartPin>) -> usize {
        part_pins.sort_by(|a, b| a.compare_to(b));
        let no = self.list.len() + 1;
        self.list.push(LogicalPart::new(name, no, part_pins));
        no
    }

    /// Port of `LogicalParts.get(String)` (LogicalParts.java:22-30): the logical part named
    /// `name` (case-insensitive), or `None`.
    pub fn get_by_name(&self, name: &str) -> Option<&LogicalPart> {
        self.list.iter().find(|p| equals_ignore_case(&p.name, name))
    }

    /// Port of `LogicalParts.get(int)` (LogicalParts.java:32-39): the logical part with this id
    /// (ids start at 1).
    ///
    /// Java performs **no bounds check** here (unlike `Padstacks.get(int)`): `Vector.elementAt`
    /// throws `ArrayIndexOutOfBoundsException` out of range, and the port panics identically via
    /// slice indexing. See `docs/java-quirks.md`. Java's inconsistent-id check
    /// (LogicalParts.java:35-37, `FRLogger.warn` only) becomes a `debug_assert_eq!`, per the
    /// global constraints' "invariant-guard logs become `debug_assert!`" rule.
    pub fn get(&self, no: usize) -> &LogicalPart {
        let result = &self.list[no - 1];
        debug_assert_eq!(
            result.no, no,
            "LogicalParts.get: inconsistent part ID (LogicalParts.java:35-37)"
        );
        result
    }

    /// Port of `LogicalParts.count` (LogicalParts.java:41-44).
    pub fn count(&self) -> usize {
        self.list.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part_pin(pin_index: i32, name: &str) -> PartPin {
        PartPin::new(pin_index, name, "U1", 1, name, 1)
    }

    #[test]
    fn add_sorts_pins_by_pin_index() {
        // LogicalParts.java:15-20: Arrays.sort(partPinArr) before storing.
        let mut parts = LogicalParts::new();
        let pins = vec![part_pin(2, "b"), part_pin(0, "a"), part_pin(1, "c")];
        let no = parts.add("U1", pins);
        let part = parts.get(no);
        assert_eq!(part.pin_count(), 3);
        assert_eq!(part.get_pin(0).unwrap().pin_name, "a");
        assert_eq!(part.get_pin(1).unwrap().pin_name, "c");
        assert_eq!(part.get_pin(2).unwrap().pin_name, "b");
    }

    #[test]
    fn add_sort_is_stable_for_equal_pin_index() {
        let mut parts = LogicalParts::new();
        let pins = vec![part_pin(0, "first"), part_pin(0, "second")];
        let no = parts.add("U1", pins);
        let part = parts.get(no);
        assert_eq!(part.get_pin(0).unwrap().pin_name, "first");
        assert_eq!(part.get_pin(1).unwrap().pin_name, "second");
    }

    #[test]
    fn get_pin_bounds_checked() {
        let mut parts = LogicalParts::new();
        let no = parts.add("U1", vec![part_pin(0, "a")]);
        let part = parts.get(no);
        assert!(part.get_pin(-1).is_none());
        assert!(part.get_pin(1).is_none());
        assert!(part.get_pin(0).is_some());
    }

    #[test]
    fn get_by_name_is_case_insensitive() {
        // LogicalParts.java:22-30.
        let mut parts = LogicalParts::new();
        parts.add("U1", vec![]);
        assert!(parts.get_by_name("u1").is_some());
        assert!(parts.get_by_name("U1").is_some());
        assert!(parts.get_by_name("u2").is_none());
    }

    #[test]
    #[should_panic]
    fn get_by_id_panics_out_of_range_like_java_elementat() {
        // LogicalParts.java:32-39 has no bounds check, unlike Padstacks.get(int).
        let parts = LogicalParts::new();
        parts.get(1);
    }

    #[test]
    fn part_pin_compare_to_orders_by_pin_index_only() {
        // LogicalPart.java:110-113.
        let a = part_pin(1, "x");
        let b = part_pin(2, "a");
        assert_eq!(a.compare_to(&b), Ordering::Less);
        assert_eq!(b.compare_to(&a), Ordering::Greater);
        assert_eq!(a.compare_to(&a.clone()), Ordering::Equal);
    }

    #[test]
    fn count_tracks_added_parts() {
        let mut parts = LogicalParts::new();
        assert_eq!(parts.count(), 0);
        parts.add("U1", vec![]);
        parts.add("U2", vec![]);
        assert_eq!(parts.count(), 2);
    }
}
