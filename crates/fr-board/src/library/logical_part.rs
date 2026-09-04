use std::cmp::Ordering;

use crate::rules::equals_ignore_case;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartPin {
    pub pin_index: i32,
    pub pin_name: String,
    pub gate_name: String,
    pub gate_swap_code: i32,
    pub gate_pin_name: String,
    pub gate_pin_swap_code: i32,
}

impl PartPin {
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

    pub fn compare_to(&self, other: &PartPin) -> Ordering {
        self.pin_index.cmp(&other.pin_index)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicalPart {
    pub name: String,
    pub no: usize,
    part_pins: Vec<PartPin>,
}

impl LogicalPart {
    pub(crate) fn new(name: impl Into<String>, no: usize, part_pins: Vec<PartPin>) -> LogicalPart {
        LogicalPart {
            name: name.into(),
            no,
            part_pins,
        }
    }

    pub fn pin_count(&self) -> usize {
        self.part_pins.len()
    }

    pub fn get_pin(&self, pin_index: i32) -> Option<&PartPin> {
        if pin_index < 0 || pin_index as usize >= self.part_pins.len() {
            return None;
        }
        Some(&self.part_pins[pin_index as usize])
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LogicalParts {
    list: Vec<LogicalPart>,
}

impl LogicalParts {
    pub fn new() -> LogicalParts {
        LogicalParts::default()
    }

    pub fn add(&mut self, name: impl Into<String>, mut part_pins: Vec<PartPin>) -> usize {
        part_pins.sort_by(|a, b| a.compare_to(b));
        let no = self.list.len() + 1;
        self.list.push(LogicalPart::new(name, no, part_pins));
        no
    }

    pub fn get_by_name(&self, name: &str) -> Option<&LogicalPart> {
        self.list.iter().find(|p| equals_ignore_case(&p.name, name))
    }

    pub fn get(&self, no: usize) -> &LogicalPart {
        let result = &self.list[no - 1];
        debug_assert_eq!(
            result.no, no,
            "LogicalParts.get: inconsistent part ID (LogicalParts.java:35-37)"
        );
        result
    }

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
        let mut parts = LogicalParts::new();
        parts.add("U1", vec![]);
        assert!(parts.get_by_name("u1").is_some());
        assert!(parts.get_by_name("U1").is_some());
        assert!(parts.get_by_name("u2").is_none());
    }

    #[test]
    #[should_panic]
    fn get_by_id_panics_out_of_range_like_java_elementat() {
        let parts = LogicalParts::new();
        parts.get(1);
    }

    #[test]
    fn part_pin_compare_to_orders_by_pin_index_only() {
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
