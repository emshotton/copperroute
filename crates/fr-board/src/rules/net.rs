use std::cmp::Ordering;
use std::fmt;

use crate::ids::NetClassId;

use super::{compare_to_ignore_case, equals_ignore_case};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Net {
        pub name: String,
            pub subnet_number: i32,
        pub net_number: i32,
            pub contains_plane: bool,
                        pub net_class: NetClassId,
}

impl Net {
            pub fn new(
        name: impl Into<String>,
        subnet_number: i32,
        number: i32,
        contains_plane: bool,
        net_class: NetClassId,
    ) -> Net {
        Net {
            name: name.into(),
            subnet_number,
            net_number: number,
            contains_plane,
            net_class,
        }
    }

        pub fn get_net_class(&self) -> NetClassId {
        self.net_class
    }

        pub fn set_class(&mut self, net_class: NetClassId) {
        self.net_class = net_class;
    }

        pub fn contains_plane(&self) -> bool {
        self.contains_plane
    }

        pub fn set_contains_plane(&mut self, value: bool) {
        self.contains_plane = value;
    }

                            pub fn compare_to(&self, other: &Net) -> Ordering {
        compare_to_ignore_case(&self.name, &other.name)
    }
}

impl fmt::Display for Net {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Net #{} ({})", self.net_number, self.name)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Nets {
        nets: Vec<Net>,
}

impl Nets {
        pub const MAX_LEGAL_NET_NUMBER: i32 = 9_999_999;

        pub const HIDDEN_NET_NUMBER: i32 = 10_000_001;

        pub fn new() -> Nets {
        Nets::default()
    }

            pub fn is_normal_net_number(net_number: i32) -> bool {
        net_number > 0 && net_number <= Nets::MAX_LEGAL_NET_NUMBER
    }

            pub fn max_net_number(&self) -> i32 {
        self.nets.len() as i32
    }

            pub fn get_by_name_and_subnet(&self, name: &str, subnet_number: i32) -> Option<&Net> {
        self.nets
            .iter()
            .find(|n| equals_ignore_case(&n.name, name) && n.subnet_number == subnet_number)
    }

            pub fn get_by_name(&self, name: &str) -> Vec<&Net> {
        self.nets
            .iter()
            .filter(|n| equals_ignore_case(&n.name, name))
            .collect()
    }

                /// Java's `FRLogger.warn("Nets.get: inconsistent netNumber")` guard becomes a `debug_assert!`
        pub fn get(&self, net_number: i32) -> Option<&Net> {
        if net_number < 1 || net_number > self.nets.len() as i32 {
            return None;
        }
        let result = &self.nets[(net_number - 1) as usize];
        debug_assert_eq!(
            result.net_number, net_number,
            "Nets.get: inconsistent netNumber (Nets.java:70-72)"
        );
        Some(result)
    }

        pub fn get_mut(&mut self, net_number: i32) -> Option<&mut Net> {
        if net_number < 1 || net_number > self.nets.len() as i32 {
            return None;
        }
        Some(&mut self.nets[(net_number - 1) as usize])
    }

                    pub fn iter(&self) -> std::slice::Iter<'_, Net> {
        self.nets.iter()
    }

            pub fn count(&self) -> usize {
        self.nets.len()
    }

                                                pub fn new_net(&mut self, net_class: NetClassId) -> &mut Net {
        let net_name = format!("net#{}", self.nets.len() + 1);
        self.add(net_name, 1, false, net_class)
    }

                            pub fn add(
        &mut self,
        name: impl Into<String>,
        subnet_number: i32,
        contains_plane: bool,
        net_class: NetClassId,
    ) -> &mut Net {
        let new_net_no = self.nets.len() as i32 + 1;
        debug_assert!(
            new_net_no < Nets::MAX_LEGAL_NET_NUMBER,
            "Nets.add_net: maxNetNo out of range (Nets.java:90-92)"
        );
        self.nets.push(Net::new(
            name,
            subnet_number,
            new_net_no,
            contains_plane,
            net_class,
        ));
        self.nets.last_mut().expect("just pushed")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFAULT_CLASS: NetClassId = NetClassId(0);

    #[test]
    fn add_numbers_nets_from_one() {
        let mut nets = Nets::new();
        assert_eq!(nets.max_net_number(), 0);
        assert_eq!(nets.add("GND", 1, false, DEFAULT_CLASS).net_number, 1);
        assert_eq!(nets.add("VCC", 1, true, DEFAULT_CLASS).net_number, 2);
        assert_eq!(nets.max_net_number(), 2);
    }

    #[test]
    fn get_is_one_based_and_bounds_checked() {
        let mut nets = Nets::new();
        nets.add("GND", 1, false, DEFAULT_CLASS);
        assert_eq!(nets.get(1).map(|n| n.name.as_str()), Some("GND"));
        assert!(nets.get(0).is_none());
        assert!(nets.get(2).is_none());
        assert!(nets.get(-5).is_none());
    }

    #[test]
    fn get_by_name_ignores_case_and_collects_subnets() {
        let mut nets = Nets::new();
        nets.add("GND", 1, false, DEFAULT_CLASS);
        nets.add("gnd", 2, false, DEFAULT_CLASS);
        nets.add("VCC", 1, false, DEFAULT_CLASS);

        assert_eq!(nets.get_by_name("gnd").len(), 2);
        assert_eq!(nets.get_by_name("GND").len(), 2);
        assert_eq!(
            nets.get_by_name_and_subnet("GND", 2).map(|n| n.net_number),
            Some(2)
        );
        assert!(nets.get_by_name_and_subnet("GND", 3).is_none());
        assert!(nets.get_by_name("missing").is_empty());
    }

    #[test]
    fn new_net_generates_the_untranslated_key_name() {
        let mut nets = Nets::new();
        assert_eq!(nets.new_net(DEFAULT_CLASS).name, "net#1");
        assert_eq!(nets.new_net(DEFAULT_CLASS).name, "net#2");
    }

    #[test]
    fn net_display_matches_java_to_string() {
        let net = Net::new("GND", 1, 7, false, DEFAULT_CLASS);
        assert_eq!(net.to_string(), "Net #7 (GND)");
    }

    #[test]
    fn compare_to_ignores_case() {
        let a = Net::new("gnd", 1, 1, false, DEFAULT_CLASS);
        let b = Net::new("GND", 1, 2, false, DEFAULT_CLASS);
        let c = Net::new("VCC", 1, 3, false, DEFAULT_CLASS);
        assert_eq!(a.compare_to(&b), std::cmp::Ordering::Equal);
        assert_eq!(a.compare_to(&c), std::cmp::Ordering::Less);
        assert_eq!(c.compare_to(&a), std::cmp::Ordering::Greater);
    }

    #[test]
    fn contains_plane_round_trips() {
        let mut net = Net::new("VCC", 1, 1, false, DEFAULT_CLASS);
        assert!(!net.contains_plane());
        net.set_contains_plane(true);
        assert!(net.contains_plane());
    }

    #[test]
    fn set_class_replaces_the_net_class_index() {
        let mut net = Net::new("GND", 1, 1, false, DEFAULT_CLASS);
        assert_eq!(net.get_net_class(), NetClassId(0));
        net.set_class(NetClassId(3));
        assert_eq!(net.get_net_class(), NetClassId(3));
    }
}
