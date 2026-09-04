use std::cmp::Ordering;
use std::fmt;

use crate::ids::{PadstackId, ViaInfoId, ViaRuleId};

use super::PadstackLookup;

#[derive(Clone, Eq)]
pub struct ViaInfo {
    name: String,
    padstack: PadstackId,
    clearance_class_index: usize,
    attach_smd_allowed: bool,
}

impl ViaInfo {
    pub fn new(
        name: impl Into<String>,
        padstack: PadstackId,
        clearance_class_index: usize,
        drill_to_smd_allowed: bool,
    ) -> ViaInfo {
        ViaInfo {
            name: name.into(),
            padstack,
            clearance_class_index,
            attach_smd_allowed: drill_to_smd_allowed,
        }
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }

    pub fn get_padstack(&self) -> PadstackId {
        self.padstack
    }

    pub fn set_padstack(&mut self, padstack: PadstackId) {
        self.padstack = padstack;
    }

    pub fn get_clearance_class_index(&self) -> usize {
        self.clearance_class_index
    }

    pub fn set_clearance_class_index(&mut self, clearance_class_index: usize) {
        self.clearance_class_index = clearance_class_index;
    }

    pub fn attach_smd_allowed(&self) -> bool {
        self.attach_smd_allowed
    }

    pub fn set_attach_smd_allowed(&mut self, attach_smd_allowed: bool) {
        self.attach_smd_allowed = attach_smd_allowed;
    }

    pub fn compare_to(&self, other: &ViaInfo) -> Ordering {
        self.name.cmp(&other.name)
    }
}

impl fmt::Debug for ViaInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ViaInfo")
            .field("name", &self.name)
            .field("padstack", &self.padstack)
            .field("clearance_class_index", &self.clearance_class_index)
            .field("attach_smd_allowed", &self.attach_smd_allowed)
            .finish()
    }
}

impl PartialEq for ViaInfo {
    fn eq(&self, other: &ViaInfo) -> bool {
        self.name == other.name
            && self.padstack == other.padstack
            && self.clearance_class_index == other.clearance_class_index
            && self.attach_smd_allowed == other.attach_smd_allowed
    }
}

impl fmt::Display for ViaInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

#[derive(Debug, Clone, Eq, Default)]
pub struct ViaInfos {
    list: Vec<ViaInfo>,
}

impl PartialEq for ViaInfos {
    fn eq(&self, other: &ViaInfos) -> bool {
        self.list == other.list
    }
}

impl ViaInfos {
    pub fn new() -> ViaInfos {
        ViaInfos::default()
    }

    pub fn add(&mut self, via_info: ViaInfo) -> bool {
        if self.name_exists(via_info.get_name()) {
            return false;
        }
        self.list.push(via_info);
        true
    }

    pub fn count(&self) -> usize {
        self.list.len()
    }

    pub fn get(&self, index: ViaInfoId) -> &ViaInfo {
        &self.list[index.0]
    }

    pub fn get_mut(&mut self, index: ViaInfoId) -> &mut ViaInfo {
        &mut self.list[index.0]
    }

    pub fn get_by_name(&self, name: &str) -> Option<&ViaInfo> {
        self.get_no(name).map(|id| self.get(id))
    }

    pub fn get_no(&self, name: &str) -> Option<ViaInfoId> {
        self.list.iter().position(|v| v.name == name).map(ViaInfoId)
    }

    pub fn iter(&self) -> std::slice::Iter<'_, ViaInfo> {
        self.list.iter()
    }

    pub fn name_exists(&self, name: &str) -> bool {
        self.get_no(name).is_some()
    }

    pub fn remove(&mut self, index: ViaInfoId) -> bool {
        if index.0 >= self.list.len() {
            return false;
        }
        self.list.remove(index.0);
        true
    }
}

impl super::BoardRules {
    pub fn replace_via_info(&mut self, old_id: ViaInfoId, new_info: ViaInfo) -> ViaInfoId {
        assert!(
            self.via_infos.remove(old_id),
            "replace_via_info: old_id {} out of range",
            old_id.0
        );
        assert!(
            self.via_infos.add(new_info),
            "replace_via_info: the replacement's name is still taken"
        );
        ViaInfoId(self.via_infos.count() - 1)
    }

    pub fn replace_via_rule(&mut self, old_id: ViaRuleId, new_rule: ViaRule) -> ViaRuleId {
        assert!(
            old_id.0 < self.via_rules.len(),
            "replace_via_rule: old_id {} out of range",
            old_id.0
        );
        self.via_rules.remove(old_id.0);
        self.via_rules.push(new_rule);
        ViaRuleId(self.via_rules.len() - 1)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViaRule {
    pub name: String,
    vias: Vec<ViaInfo>,
}

impl ViaRule {
    pub fn new(name: impl Into<String>) -> ViaRule {
        ViaRule {
            name: name.into(),
            vias: Vec::new(),
        }
    }

    pub fn empty() -> ViaRule {
        ViaRule::new("empty")
    }

    pub fn append_via(&mut self, via: ViaInfo) {
        self.vias.push(via);
    }

    pub fn remove_via(&mut self, via: &ViaInfo) -> bool {
        match self.vias.iter().position(|v| v == via) {
            Some(index) => {
                self.vias.remove(index);
                true
            }
            None => false,
        }
    }

    pub fn via_count(&self) -> usize {
        self.vias.len()
    }

    pub fn get_via(&self, index: usize) -> &ViaInfo {
        &self.vias[index]
    }

    pub fn iter(&self) -> std::slice::Iter<'_, ViaInfo> {
        self.vias.iter()
    }

    pub fn contains(&self, via_info: &ViaInfo) -> bool {
        self.vias.iter().any(|v| v == via_info)
    }

    pub fn contains_padstack(&self, padstack: PadstackId) -> bool {
        self.vias.iter().any(|v| v.get_padstack() == padstack)
    }

    pub fn get_layer_range(
        &self,
        from_layer: i32,
        to_layer: i32,
        padstacks: &impl PadstackLookup,
    ) -> Option<&ViaInfo> {
        self.vias.iter().find(|via| {
            let padstack = via.get_padstack();
            padstacks.padstack_from_layer(padstack) == from_layer
                && padstacks.padstack_to_layer(padstack) == to_layer
        })
    }
}

impl fmt::Display for ViaRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestPadstacks;

    impl PadstackLookup for TestPadstacks {
        fn padstack_from_layer(&self, padstack: PadstackId) -> i32 {
            padstack.0 as i32
        }
        fn padstack_to_layer(&self, padstack: PadstackId) -> i32 {
            padstack.0 as i32 + 1
        }
        fn padstack_shape_max_width(&self, padstack: PadstackId, _layer: i32) -> Option<f64> {
            Some(10.0 * (padstack.0 as f64 + 1.0))
        }
    }

    fn via_infos() -> ViaInfos {
        let mut infos = ViaInfos::new();
        infos.add(ViaInfo::new("via0", PadstackId(0), 1, false));
        infos.add(ViaInfo::new("via1", PadstackId(1), 1, true));
        infos
    }

    #[test]
    fn add_rejects_duplicate_names() {
        let mut infos = via_infos();
        assert_eq!(infos.count(), 2);
        assert!(!infos.add(ViaInfo::new("via0", PadstackId(5), 2, false)));
        assert_eq!(infos.count(), 2);
        assert_eq!(infos.get(ViaInfoId(0)).get_padstack(), PadstackId(0));
    }

    #[test]
    fn name_lookup_is_case_sensitive() {
        let infos = via_infos();
        assert_eq!(infos.get_no("via1"), Some(ViaInfoId(1)));
        assert_eq!(infos.get_no("VIA1"), None);
        assert!(infos.name_exists("via0"));
        assert!(!infos.name_exists("via2"));
        assert_eq!(
            infos.get_by_name("via1").map(ViaInfo::get_name),
            Some("via1")
        );
    }

    #[test]
    fn remove_reports_whether_the_via_was_present() {
        let mut infos = via_infos();
        assert!(infos.remove(ViaInfoId(0)));
        assert_eq!(infos.count(), 1);
        assert_eq!(infos.get(ViaInfoId(0)).get_name(), "via1");
        assert!(!infos.remove(ViaInfoId(7)));
    }

    #[test]
    fn replace_via_info_leaves_every_rule_alone() {
        let layer_structure = crate::structure::LayerStructure::new(vec![
            crate::structure::Layer::new("F.Cu", true),
            crate::structure::Layer::new("B.Cu", true),
        ]);
        let clearance_matrix =
            crate::rules::ClearanceMatrix::new(2, &layer_structure, &["null", "default"]);
        let mut rules = super::super::BoardRules::new(layer_structure, clearance_matrix);
        rules
            .via_infos
            .add(ViaInfo::new("A", PadstackId(1), 1, false));
        rules
            .via_infos
            .add(ViaInfo::new("B", PadstackId(2), 1, false));
        rules
            .via_infos
            .add(ViaInfo::new("C", PadstackId(3), 1, false));
        let mut rule = ViaRule::new("r");
        rule.append_via(rules.via_infos.get(ViaInfoId(0)).clone());
        rule.append_via(rules.via_infos.get(ViaInfoId(2)).clone());
        rule.append_via(rules.via_infos.get(ViaInfoId(1)).clone());
        rules.via_rules.push(rule);

        let old_id = rules.via_infos.get_no("A").expect("A is present");
        let new_id = rules.replace_via_info(old_id, ViaInfo::new("A", PadstackId(9), 2, true));

        assert_eq!(new_id, ViaInfoId(2));
        let names: Vec<&str> = rules.via_infos.iter().map(ViaInfo::get_name).collect();
        assert_eq!(names, ["B", "C", "A"]);
        assert_eq!(rules.via_infos.get(new_id).get_padstack(), PadstackId(9));

        let rule = &rules.via_rules[0];
        let resolved: Vec<&str> = rule.iter().map(ViaInfo::get_name).collect();
        assert_eq!(resolved, ["A", "C", "B"]);
        assert_eq!(rule.get_via(0).get_padstack(), PadstackId(1));
        assert_eq!(rule.get_via(0).get_clearance_class_index(), 1);
        assert!(!rule.get_via(0).attach_smd_allowed());
    }

    #[test]
    fn a_via_rule_holds_its_own_copy() {
        let mut infos = via_infos();
        let mut rule = ViaRule::new("r");
        rule.append_via(infos.get(ViaInfoId(0)).clone());

        infos.get_mut(ViaInfoId(0)).set_attach_smd_allowed(true);
        infos.get_mut(ViaInfoId(0)).set_padstack(PadstackId(7));
        infos.get_mut(ViaInfoId(0)).set_clearance_class_index(9);

        assert!(!rule.get_via(0).attach_smd_allowed());
        assert_eq!(rule.get_via(0).get_padstack(), PadstackId(0));
        assert_eq!(rule.get_via(0).get_clearance_class_index(), 1);
    }

    #[test]
    fn remove_via_removes_the_first_equal_element() {
        let a = ViaInfo::new("a", PadstackId(0), 1, false);
        let b = ViaInfo::new("b", PadstackId(1), 1, false);
        let mut rule = ViaRule::new("r");
        rule.append_via(a.clone());
        rule.append_via(b.clone());
        rule.append_via(a.clone());
        assert_eq!(rule.via_count(), 3);

        assert!(rule.remove_via(&a));
        assert_eq!(
            rule.iter().map(ViaInfo::get_name).collect::<Vec<_>>(),
            ["b", "a"],
            "only the first equal element goes"
        );
        assert!(rule.remove_via(&a));
        assert!(!rule.remove_via(&a));
    }

    #[test]
    fn a_rule_cannot_hold_two_equal_via_infos() {
        let layer_structure = crate::structure::LayerStructure::new(vec![
            crate::structure::Layer::new("F.Cu", true),
            crate::structure::Layer::new("B.Cu", true),
        ]);
        let clearance_matrix =
            crate::rules::ClearanceMatrix::new(2, &layer_structure, &["null", "default"]);
        let mut rules = super::super::BoardRules::new(layer_structure.clone(), clearance_matrix);
        assert!(
            rules
                .via_infos
                .add(ViaInfo::new("A", PadstackId(0), 1, false))
        );
        assert!(
            rules
                .via_infos
                .add(ViaInfo::new("B", PadstackId(0), 1, false))
        );
        assert!(
            rules
                .via_infos
                .add(ViaInfo::new("C", PadstackId(1), 1, false))
        );
        assert!(
            !rules
                .via_infos
                .add(ViaInfo::new("A", PadstackId(3), 1, false))
        );

        let net_class = rules.net_classes.append("default", &layer_structure, false);
        rules.create_default_via_rule(net_class, "default", &TestPadstacks);

        let rule = &rules.via_rules[0];
        for i in 0..rule.via_count() {
            for j in 0..i {
                assert_ne!(
                    rule.get_via(i),
                    rule.get_via(j),
                    "a via rule holds no two equal ViaInfos, so value equality is reference \
                     equality for `remove_via`"
                );
            }
        }
    }

    #[test]
    fn replace_via_rule_leaves_every_net_class_holding_what_it_held() {
        let layer_structure = crate::structure::LayerStructure::new(vec![
            crate::structure::Layer::new("F.Cu", true),
            crate::structure::Layer::new("B.Cu", true),
        ]);
        let clearance_matrix =
            crate::rules::ClearanceMatrix::new(2, &layer_structure, &["null", "default"]);
        let mut rules = super::super::BoardRules::new(layer_structure.clone(), clearance_matrix);
        let mut original = ViaRule::new("default");
        rules
            .via_infos
            .add(ViaInfo::new("A", PadstackId(0), 1, false));
        original.append_via(rules.via_infos.get(ViaInfoId(0)).clone());
        rules.via_rules.push(original.clone());
        rules.via_rules.push(ViaRule::new("wide"));
        rules.via_rules.push(ViaRule::new("narrow"));

        let a = rules.net_classes.append("a", &layer_structure, false);
        let b = rules.net_classes.append("b", &layer_structure, false);
        let c = rules.net_classes.append("c", &layer_structure, false);
        rules.net_classes.get_mut(a).set_via_rule(Some(original));
        rules
            .net_classes
            .get_mut(b)
            .set_via_rule(Some(ViaRule::new("narrow")));
        rules.net_classes.get_mut(c).set_via_rule(None);

        let new_id = rules.replace_via_rule(ViaRuleId(0), ViaRule::new("default"));

        assert_eq!(new_id, ViaRuleId(2));
        let names: Vec<&str> = rules.via_rules.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["wide", "narrow", "default"]);
        let a_rule = rules.net_classes.get(a).get_via_rule().expect("a's rule");
        assert_eq!(a_rule.name, "default");
        assert_eq!(a_rule.via_count(), 1);
        assert_eq!(rules.via_rules[2].via_count(), 0);
        assert_eq!(
            rules.net_classes.get(b).get_via_rule().map(|r| &r.name),
            Some(&"narrow".to_string())
        );
        assert_eq!(rules.net_classes.get(c).get_via_rule(), None);
    }

    #[test]
    fn via_info_accessors_round_trip() {
        let mut info = ViaInfo::new("v", PadstackId(2), 3, false);
        assert_eq!(info.get_name(), "v");
        assert_eq!(info.get_padstack(), PadstackId(2));
        assert_eq!(info.get_clearance_class_index(), 3);
        assert!(!info.attach_smd_allowed());
        assert_eq!(info.to_string(), "v");

        info.set_name("w");
        info.set_padstack(PadstackId(4));
        info.set_clearance_class_index(5);
        info.set_attach_smd_allowed(true);
        assert_eq!(info.get_name(), "w");
        assert_eq!(info.get_padstack(), PadstackId(4));
        assert_eq!(info.get_clearance_class_index(), 5);
        assert!(info.attach_smd_allowed());
    }

    #[test]
    fn via_info_equality_is_by_value() {
        let base = ViaInfo::new("via", PadstackId(1), 2, false);
        assert_eq!(base, ViaInfo::new("via", PadstackId(1), 2, false));
        assert_ne!(base, ViaInfo::new("other", PadstackId(1), 2, false));
        assert_ne!(base, ViaInfo::new("via", PadstackId(2), 2, false));
        assert_ne!(base, ViaInfo::new("via", PadstackId(1), 3, false));
        assert_ne!(base, ViaInfo::new("via", PadstackId(1), 2, true));
    }

    #[test]
    fn via_info_compare_to_is_case_sensitive() {
        let a = ViaInfo::new("Via", PadstackId(0), 0, false);
        let b = ViaInfo::new("via", PadstackId(0), 0, false);
        assert_eq!(a.compare_to(&b), Ordering::Less);
        assert_eq!(a.compare_to(&a.clone()), Ordering::Equal);
    }

    #[test]
    fn via_rule_append_remove_and_contains() {
        let infos = via_infos();
        let (v0, v1) = (infos.get(ViaInfoId(0)), infos.get(ViaInfoId(1)));
        let absent = ViaInfo::new("via2", PadstackId(2), 1, false);
        let mut rule = ViaRule::new("default");
        assert_eq!(rule.name, "default");
        rule.append_via(v0.clone());
        rule.append_via(v1.clone());
        assert_eq!(rule.via_count(), 2);
        assert_eq!(rule.get_via(0), v0);
        assert!(rule.contains(v1));
        assert!(!rule.contains(&absent));

        assert!(rule.remove_via(v0));
        assert_eq!(rule.via_count(), 1);
        assert_eq!(rule.get_via(0), v1);
        assert!(!rule.remove_via(v0));
    }

    #[test]
    fn contains_padstack_reads_the_held_via() {
        let infos = via_infos();
        let mut rule = ViaRule::new("default");
        rule.append_via(infos.get(ViaInfoId(1)).clone());
        assert!(rule.contains_padstack(PadstackId(1)));
        assert!(!rule.contains_padstack(PadstackId(0)));
    }

    #[test]
    fn get_layer_range_finds_the_first_matching_via() {
        let infos = via_infos();
        let mut rule = ViaRule::new("default");
        rule.append_via(infos.get(ViaInfoId(0)).clone());
        rule.append_via(infos.get(ViaInfoId(1)).clone());

        assert_eq!(
            rule.get_layer_range(0, 1, &TestPadstacks)
                .map(ViaInfo::get_name),
            Some("via0")
        );
        assert_eq!(
            rule.get_layer_range(1, 2, &TestPadstacks)
                .map(ViaInfo::get_name),
            Some("via1")
        );
        assert!(rule.get_layer_range(3, 4, &TestPadstacks).is_none());
    }
}
