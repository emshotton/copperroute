use std::collections::HashMap;

use crate::ids::ItemId;

#[derive(Debug, Clone, PartialEq, Eq)]
struct NetTiePad {
    footprint: String,
    nets: Vec<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NetTies {
    pads: HashMap<ItemId, NetTiePad>,
    by_footprint: HashMap<String, Vec<ItemId>>,
}

impl NetTies {
    pub fn register_pad(&mut self, pad: ItemId, footprint: String, nets: Vec<i32>) {
        self.by_footprint
            .entry(footprint.clone())
            .or_default()
            .push(pad);
        self.pads.insert(pad, NetTiePad { footprint, nets });
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pads.is_empty()
    }

    #[must_use]
    pub fn pads_of(&self, footprint: &str) -> &[ItemId] {
        self.by_footprint
            .get(footprint)
            .map_or(&[][..], Vec::as_slice)
    }

    #[must_use]
    pub fn nets_of(&self, pad: ItemId) -> &[i32] {
        self.pads.get(&pad).map_or(&[][..], |p| p.nets.as_slice())
    }

    #[must_use]
    pub fn may_short(&self, a: ItemId, a_nets: &[i32], b: ItemId, b_nets: &[i32]) -> bool {
        if self.pads.is_empty() {
            return false;
        }
        let (Some(pad_a), Some(pad_b)) = (self.pads.get(&a), self.pads.get(&b)) else {
            return false;
        };
        pad_a.footprint == pad_b.footprint
            && b_nets.iter().any(|net| pad_a.nets.contains(net))
            && a_nets.iter().any(|net| pad_b.nets.contains(net))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_registry_permits_nothing() {
        let ties = NetTies::default();
        assert!(ties.is_empty());
        assert!(!ties.may_short(ItemId(1), &[1], ItemId(2), &[2]));
        assert!(ties.nets_of(ItemId(1)).is_empty());
        assert!(ties.pads_of("0").is_empty());
    }

    #[test]
    fn two_registered_pads_of_one_footprint_may_short_their_groups_nets() {
        let mut ties = NetTies::default();
        ties.register_pad(ItemId(1), "0".to_string(), vec![2]);
        ties.register_pad(ItemId(2), "0".to_string(), vec![1]);
        assert!(!ties.is_empty());
        assert!(ties.may_short(ItemId(1), &[1], ItemId(2), &[2]));
        assert!(ties.may_short(ItemId(2), &[2], ItemId(1), &[1]));
    }

    #[test]
    fn pads_of_different_footprints_never_short() {
        let mut ties = NetTies::default();
        ties.register_pad(ItemId(1), "0".to_string(), vec![2]);
        ties.register_pad(ItemId(2), "7".to_string(), vec![1]);
        assert!(!ties.may_short(ItemId(1), &[1], ItemId(2), &[2]));
    }

    #[test]
    fn pads_of_different_groups_in_one_footprint_never_short() {
        let mut ties = NetTies::default();
        ties.register_pad(ItemId(1), "0".to_string(), vec![2]);
        ties.register_pad(ItemId(2), "0".to_string(), vec![1]);
        ties.register_pad(ItemId(3), "0".to_string(), vec![4]);
        ties.register_pad(ItemId(4), "0".to_string(), vec![3]);
        assert!(!ties.may_short(ItemId(1), &[1], ItemId(3), &[3]));
    }

    #[test]
    fn an_unregistered_pad_never_shorts() {
        let mut ties = NetTies::default();
        ties.register_pad(ItemId(1), "0".to_string(), vec![2]);
        assert!(!ties.may_short(ItemId(1), &[1], ItemId(9), &[2]));
    }

    #[test]
    fn pads_of_lists_a_footprints_registered_pads_in_registration_order() {
        let mut ties = NetTies::default();
        ties.register_pad(ItemId(5), "3".to_string(), vec![2]);
        ties.register_pad(ItemId(6), "3".to_string(), vec![1]);
        assert_eq!(ties.pads_of("3"), &[ItemId(5), ItemId(6)]);
        assert_eq!(ties.nets_of(ItemId(5)), &[2]);
    }
}
