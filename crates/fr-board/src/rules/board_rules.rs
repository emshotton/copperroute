use crate::ids::{NetClassId, ViaInfoId, ViaRuleId};
use crate::structure::{AngleRestriction, LayerStructure};

use super::{
    ClearanceClassIndexed, ClearanceMatrix, DrcConstraints, ItemClass, NetClasses, Nets,
    PadstackLookup, ViaInfos, ViaRule,
};

#[derive(Debug, Clone, PartialEq)]
pub struct BoardRules {
    pub clearance_matrix: ClearanceMatrix,
    pub nets: Nets,
    pub via_infos: ViaInfos,
    pub via_rules: Vec<ViaRule>,
    pub net_classes: NetClasses,
    pub drc_constraints: Option<DrcConstraints>,
    layer_structure: LayerStructure,
    pub trace_angle_restriction: AngleRestriction,
    ignore_conduction: bool,
    min_trace_half_width: i32,
    max_trace_half_width: i32,
    pin_edge_to_turn_dist: f64,
    use_slow_autoroute_algorithm: bool,
    hole_clearance: i32,
}

impl BoardRules {
    pub fn new(layer_structure: LayerStructure, clearance_matrix: ClearanceMatrix) -> BoardRules {
        BoardRules {
            clearance_matrix,
            nets: Nets::new(),
            via_infos: ViaInfos::new(),
            via_rules: Vec::new(),
            net_classes: NetClasses::new(),
            drc_constraints: None,
            layer_structure,
            trace_angle_restriction: AngleRestriction::FortyFiveDegree,
            ignore_conduction: true,
            min_trace_half_width: 100_000,
            max_trace_half_width: 100,
            pin_edge_to_turn_dist: 0.0,
            use_slow_autoroute_algorithm: false,
            hole_clearance: 0,
        }
    }

    pub fn layer_structure(&self) -> &LayerStructure {
        &self.layer_structure
    }

    pub fn default_clearance_class() -> usize {
        1
    }

    pub fn clearance_class_none() -> usize {
        0
    }

    pub fn get_trace_half_width(&self, net_number: i32, layer: usize) -> i32 {
        let net = self
            .nets
            .get(net_number)
            .expect("BoardRules.getTraceHalfWidth: unknown net (Java NPEs at BoardRules.java:76)");
        self.net_classes
            .get(net.net_class)
            .get_trace_half_width(layer)
    }

    pub fn trace_widths_are_layer_dependent(&self, net_number: i32) -> bool {
        let compare_width = self.get_trace_half_width(net_number, 0);
        (1..self.layer_structure.count())
            .any(|i| self.get_trace_half_width(net_number, i) != compare_width)
    }

    pub fn get_min_trace_half_width(&self) -> i32 {
        self.min_trace_half_width
    }

    pub fn get_max_trace_half_width(&self) -> i32 {
        self.max_trace_half_width
    }

    pub fn get_hole_clearance(&self) -> i32 {
        self.hole_clearance
    }

    pub fn set_hole_clearance(&mut self, value: i32) {
        self.hole_clearance = value.max(0);
    }

    pub fn set_default_trace_half_width(&mut self, layer: usize, value: i32) {
        let default_class = self.get_default_net_class();
        self.net_classes
            .get_mut(default_class)
            .set_trace_half_width(layer, value);
        self.min_trace_half_width = self.min_trace_half_width.min(value);
        self.max_trace_half_width = self.max_trace_half_width.max(value);
    }

    pub fn get_default_trace_half_width(&mut self, layer: usize) -> i32 {
        let default_class = self.get_default_net_class();
        self.net_classes
            .get(default_class)
            .get_trace_half_width(layer)
    }

    pub fn set_default_trace_half_widths(&mut self, value: i32) {
        if value <= 0 {
            // FRLogger.warn("BoardRules.set_trace_half_widths: value out of range")
            return;
        }
        let default_class = self.get_default_net_class();
        self.net_classes
            .get_mut(default_class)
            .set_trace_half_width_on_all_layers(value);
        self.min_trace_half_width = self.min_trace_half_width.min(value);
        self.max_trace_half_width = self.max_trace_half_width.max(value);
    }

    pub fn get_default_net_class(&mut self) -> NetClassId {
        if self.net_classes.count() == 0 {
            self.create_default_net_class();
        }
        NetClassId(0)
    }

    pub fn create_default_net_class(&mut self) {
        let id = self
            .net_classes
            .append("default", &self.layer_structure, false);
        let default_trace_half_width = 1500;
        let net_class = self.net_classes.get_mut(id);
        net_class.set_trace_half_width_on_all_layers(default_trace_half_width);
        net_class.set_trace_clearance_class(1);
    }

    pub fn get_new_net_class(&mut self) -> NetClassId {
        let result = self
            .net_classes
            .append_with_generated_name(&self.layer_structure);
        self.initialize_new_net_class(result);
        result
    }

    pub fn get_new_net_class_named(&mut self, name: impl Into<String>) -> NetClassId {
        let result = self.net_classes.append(name, &self.layer_structure, false);
        self.initialize_new_net_class(result);
        result
    }

    fn initialize_new_net_class(&mut self, result: NetClassId) {
        let default_class = self.get_default_net_class();
        let trace_clearance_class = self
            .net_classes
            .get(default_class)
            .get_trace_clearance_class();
        let trace_half_width = self.net_classes.get(default_class).get_trace_half_width(0);
        let default_via_rule = self.get_default_via_rule().cloned();
        let net_class = self.net_classes.get_mut(result);
        net_class.set_trace_clearance_class(trace_clearance_class);
        net_class.set_via_rule(default_via_rule);
        net_class.set_trace_half_width_on_all_layers(trace_half_width);
    }

    pub fn append_net_class(&mut self) -> NetClassId {
        let new_class = self
            .net_classes
            .append_with_generated_name(&self.layer_structure);
        let default_class = self.net_classes.get(NetClassId(0));
        let via_rule = default_class.get_via_rule().cloned();
        let trace_half_width = default_class.get_trace_half_width(0);
        let trace_clearance_class = default_class.get_trace_clearance_class();
        let new = self.net_classes.get_mut(new_class);
        new.set_via_rule(via_rule);
        new.set_trace_half_width_on_all_layers(trace_half_width);
        new.set_trace_clearance_class(trace_clearance_class);
        new_class
    }

    pub fn append_net_class_named(&mut self, name: &str) -> NetClassId {
        if let Some(found) = self.net_classes.get_no(name) {
            return found;
        }
        let new_class = self.net_classes.append(name, &self.layer_structure, false);
        let default_class = self.net_classes.get(NetClassId(0));
        let default_item_clearance_classes = default_class.default_item_clearance_classes;
        let via_rule = default_class.get_via_rule().cloned();
        let trace_half_width = default_class.get_trace_half_width(0);
        let trace_clearance_class = default_class.get_trace_clearance_class();
        let new = self.net_classes.get_mut(new_class);
        new.default_item_clearance_classes = default_item_clearance_classes;
        new.set_via_rule(via_rule);
        new.set_trace_half_width_on_all_layers(trace_half_width);
        new.set_trace_clearance_class(trace_clearance_class);
        new_class
    }

    pub fn create_default_via_rule(
        &mut self,
        net_class: NetClassId,
        name: impl Into<String>,
        padstacks: &impl PadstackLookup,
    ) {
        if self.via_infos.count() == 0 {
            return;
        }
        let mut default_rule = ViaRule::new(name);
        let default_via_cl_class = self
            .net_classes
            .get(net_class)
            .default_item_clearance_classes
            .get(ItemClass::Via);
        for i in 0..self.via_infos.count() {
            let info = self.via_infos.get(ViaInfoId(i));
            if info.get_clearance_class_index() != default_via_cl_class {
                continue;
            }
            let current_padstack = info.get_padstack();
            let current_from_layer = padstacks.padstack_from_layer(current_padstack);
            let current_to_layer = padstacks.padstack_to_layer(current_padstack);
            let existing_via = default_rule
                .get_layer_range(current_from_layer, current_to_layer, padstacks)
                .cloned();
            match existing_via {
                Some(existing) => {
                    let new_width = padstacks
                        .padstack_shape_max_width(current_padstack, current_from_layer)
                        .expect("padstack has a shape on its own fromLayer");
                    let existing_width = padstacks
                        .padstack_shape_max_width(existing.get_padstack(), current_from_layer)
                        .expect("padstack has a shape on the matched fromLayer");
                    if new_width < existing_width {
                        default_rule.remove_via(&existing);
                        default_rule.append_via(info.clone());
                    }
                }
                None => default_rule.append_via(info.clone()),
            }
        }
        self.net_classes
            .get_mut(net_class)
            .set_via_rule(Some(default_rule.clone()));
        self.via_rules.push(default_rule);
    }

    pub fn get_default_via_rule(&self) -> Option<&ViaRule> {
        self.via_rules.first()
    }

    pub fn get_default_via_rule_id(&self) -> Option<ViaRuleId> {
        if self.via_rules.is_empty() {
            None
        } else {
            Some(ViaRuleId(0))
        }
    }

    pub fn get_via_rule(&self, name: &str) -> Option<ViaRuleId> {
        self.via_rules
            .iter()
            .position(|r| r.name == name)
            .map(ViaRuleId)
    }

    pub fn change_clearance_class_index<'a, T: ClearanceClassIndexed + 'a>(
        &mut self,
        from_index: usize,
        to_index: usize,
        board_items: impl IntoIterator<Item = &'a mut T>,
    ) {
        for item in board_items {
            if item.clearance_class_index() == from_index {
                item.set_clearance_class_index(to_index);
            }
        }

        for i in 0..self.net_classes.count() {
            let net_class = self.net_classes.get_mut(NetClassId(i));
            if net_class.get_trace_clearance_class() == from_index {
                net_class.set_trace_clearance_class(to_index);
            }
            for item_class in ItemClass::VALUES {
                if net_class.default_item_clearance_classes.get(item_class) == from_index {
                    net_class
                        .default_item_clearance_classes
                        .set(item_class, to_index);
                }
            }
        }

        for i in 0..self.via_infos.count() {
            let via = self.via_infos.get_mut(ViaInfoId(i));
            if via.get_clearance_class_index() == from_index {
                via.set_clearance_class_index(to_index);
            }
        }
    }

    pub fn remove_clearance_class<'a, T: ClearanceClassIndexed + 'a>(
        &mut self,
        index: usize,
        board_items: impl IntoIterator<Item = &'a mut T>,
    ) -> bool {
        let mut board_items: Vec<&mut T> = board_items.into_iter().collect();

        if board_items
            .iter()
            .any(|item| item.clearance_class_index() == index)
        {
            return false;
        }
        for i in 0..self.net_classes.count() {
            let net_class = self.net_classes.get(NetClassId(i));
            if net_class.get_trace_clearance_class() == index {
                return false;
            }
            for item_class in ItemClass::VALUES {
                if net_class.default_item_clearance_classes.get(item_class) == index {
                    return false;
                }
            }
        }
        for i in 0..self.via_infos.count() {
            if self.via_infos.get(ViaInfoId(i)).get_clearance_class_index() == index {
                return false;
            }
        }

        for item in &mut board_items {
            if item.clearance_class_index() > index {
                item.set_clearance_class_index(item.clearance_class_index() - 1);
            }
        }
        for i in 0..self.net_classes.count() {
            let net_class = self.net_classes.get_mut(NetClassId(i));
            if net_class.get_trace_clearance_class() > index {
                net_class.set_trace_clearance_class(net_class.get_trace_clearance_class() - 1);
            }
            for item_class in ItemClass::VALUES {
                let current_class_no = net_class.default_item_clearance_classes.get(item_class);
                if current_class_no > index {
                    net_class
                        .default_item_clearance_classes
                        .set(item_class, current_class_no - 1);
                }
            }
        }
        for i in 0..self.via_infos.count() {
            let via = self.via_infos.get_mut(ViaInfoId(i));
            if via.get_clearance_class_index() > index {
                via.set_clearance_class_index(via.get_clearance_class_index() - 1);
            }
        }
        self.clearance_matrix.remove_class(index);
        true
    }

    pub fn get_pin_edge_to_turn_dist(&self) -> f64 {
        self.pin_edge_to_turn_dist
    }

    pub fn set_pin_edge_to_turn_dist(&mut self, value: f64) {
        self.pin_edge_to_turn_dist = value;
    }

    pub fn get_ignore_conduction(&self) -> bool {
        self.ignore_conduction
    }

    pub fn set_ignore_conduction(&mut self, value: bool) {
        self.ignore_conduction = value;
    }

    pub fn get_use_slow_autoroute_algorithm(&self) -> bool {
        self.use_slow_autoroute_algorithm
    }

    pub fn set_use_slow_autoroute_algorithm(&mut self, value: bool) {
        self.use_slow_autoroute_algorithm = value;
    }

    pub fn get_default_via_diameter(&self, padstacks: &impl PadstackLookup) -> f64 {
        let Some(default_via_rule) = self.get_default_via_rule() else {
            return 0.0;
        };
        if default_via_rule.via_count() == 0 {
            return 0.0;
        }
        let via_padstack = default_via_rule.get_via(0).get_padstack();
        let from_layer = padstacks.padstack_from_layer(via_padstack);
        let to_layer = padstacks.padstack_to_layer(via_padstack);
        let result = padstacks
            .padstack_shape_max_width(via_padstack, from_layer)
            .expect("BoardRules.getDefaultViaDiameter: Java NPEs on a shapeless padstack");
        let to_width = padstacks
            .padstack_shape_max_width(via_padstack, to_layer)
            .expect("BoardRules.getDefaultViaDiameter: Java NPEs on a shapeless padstack");
        result.max(to_width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::PadstackId;
    use crate::rules::ViaInfo;
    use crate::structure::Layer;

    struct TestPadstacks;

    impl PadstackLookup for TestPadstacks {
        fn padstack_from_layer(&self, padstack: PadstackId) -> i32 {
            padstack.0 as i32 % 2
        }
        fn padstack_to_layer(&self, padstack: PadstackId) -> i32 {
            padstack.0 as i32 % 2 + 1
        }
        fn padstack_shape_max_width(&self, padstack: PadstackId, _layer: i32) -> Option<f64> {
            Some(100.0 - 10.0 * padstack.0 as f64)
        }
    }

    struct TestItem {
        clearance_class_index: usize,
    }

    impl ClearanceClassIndexed for TestItem {
        fn clearance_class_index(&self) -> usize {
            self.clearance_class_index
        }
        fn set_clearance_class_index(&mut self, index: usize) {
            self.clearance_class_index = index;
        }
    }

    fn layers() -> LayerStructure {
        LayerStructure::new(vec![Layer::new("Top", true), Layer::new("Bottom", true)])
    }

    fn rules() -> BoardRules {
        let layers = layers();
        let matrix = ClearanceMatrix::get_default_instance(&layers, 20);
        BoardRules::new(layers, matrix)
    }

    #[test]
    fn constructor_seeds_match_java() {
        let rules = rules();
        assert_eq!(
            rules.trace_angle_restriction,
            AngleRestriction::FortyFiveDegree
        );
        assert!(rules.get_ignore_conduction());
        assert_eq!(rules.get_min_trace_half_width(), 100_000);
        assert_eq!(rules.get_max_trace_half_width(), 100);
        assert_eq!(rules.get_hole_clearance(), 0);
        assert_eq!(rules.get_pin_edge_to_turn_dist(), 0.0);
        assert!(!rules.get_use_slow_autoroute_algorithm());
        assert_eq!(rules.net_classes.count(), 0);
        assert!(rules.via_rules.is_empty());
    }

    #[test]
    fn static_clearance_class_constants() {
        assert_eq!(BoardRules::default_clearance_class(), 1);
        assert_eq!(BoardRules::clearance_class_none(), 0);
    }

    #[test]
    fn get_default_net_class_creates_it_lazily() {
        let mut rules = rules();
        let default_class = rules.get_default_net_class();
        assert_eq!(default_class, NetClassId(0));
        assert_eq!(rules.net_classes.count(), 1);
        assert_eq!(rules.net_classes.get(default_class).get_name(), "default");
        assert_eq!(
            rules.net_classes.get(default_class).get_trace_half_width(0),
            1500
        );
        assert_eq!(
            rules
                .net_classes
                .get(default_class)
                .get_trace_clearance_class(),
            1
        );
        assert_eq!(rules.get_default_net_class(), NetClassId(0));
        assert_eq!(rules.net_classes.count(), 1);
    }

    #[test]
    fn set_default_trace_half_width_updates_the_running_min_and_max() {
        let mut rules = rules();
        rules.set_default_trace_half_width(0, 500);
        assert_eq!(rules.get_default_trace_half_width(0), 500);
        assert_eq!(rules.get_min_trace_half_width(), 500);
        assert_eq!(rules.get_max_trace_half_width(), 500);

        rules.set_default_trace_half_width(1, 900);
        assert_eq!(rules.get_min_trace_half_width(), 500);
        assert_eq!(rules.get_max_trace_half_width(), 900);
        assert_eq!(rules.get_default_trace_half_width(0), 500);
    }

    #[test]
    fn set_default_trace_half_widths_rejects_non_positive_values() {
        let mut rules = rules();
        rules.set_default_trace_half_widths(300);
        assert_eq!(rules.get_default_trace_half_width(0), 300);
        assert_eq!(rules.get_default_trace_half_width(1), 300);

        rules.set_default_trace_half_widths(0);
        assert_eq!(rules.get_default_trace_half_width(0), 300);
        assert_eq!(rules.get_min_trace_half_width(), 300);
        rules.set_default_trace_half_widths(-5);
        assert_eq!(rules.get_default_trace_half_width(0), 300);
    }

    #[test]
    fn hole_clearance_clamps_to_zero() {
        let mut rules = rules();
        rules.set_hole_clearance(42);
        assert_eq!(rules.get_hole_clearance(), 42);
        rules.set_hole_clearance(-1);
        assert_eq!(rules.get_hole_clearance(), 0);
    }

    #[test]
    fn get_trace_half_width_goes_through_the_nets_net_class() {
        let mut rules = rules();
        let default_class = rules.get_default_net_class();
        rules.nets.add("GND", 1, false, default_class);
        assert_eq!(rules.get_trace_half_width(1, 0), 1500);
        assert!(!rules.trace_widths_are_layer_dependent(1));

        rules.set_default_trace_half_width(1, 700);
        assert_eq!(rules.get_trace_half_width(1, 1), 700);
        assert!(rules.trace_widths_are_layer_dependent(1));
    }

    #[test]
    #[should_panic(expected = "unknown net")]
    fn get_trace_half_width_panics_on_unknown_net() {
        let rules = rules();
        rules.get_trace_half_width(1, 0);
    }

    #[test]
    fn new_net_class_inherits_from_the_default_class() {
        let mut rules = rules();
        rules.set_default_trace_half_widths(250);
        let generated = rules.get_new_net_class();
        assert_eq!(rules.net_classes.get(generated).get_name(), "class1");
        assert_eq!(
            rules.net_classes.get(generated).get_trace_half_width(1),
            250
        );
        assert_eq!(
            rules.net_classes.get(generated).get_trace_clearance_class(),
            1
        );
        assert_eq!(rules.net_classes.get(generated).get_via_rule(), None);

        let named = rules.get_new_net_class_named("power");
        assert_eq!(rules.net_classes.get(named).get_name(), "power");
        assert_eq!(rules.net_classes.get(named).get_trace_half_width(0), 250);
    }

    #[test]
    fn append_net_class_named_returns_an_existing_class_untouched() {
        let mut rules = rules();
        rules.get_default_net_class();
        let first = rules.append_net_class_named("power");
        rules
            .net_classes
            .get_mut(first)
            .set_trace_half_width_on_all_layers(77);
        let second = rules.append_net_class_named("power");
        assert_eq!(first, second);
        assert_eq!(rules.net_classes.get(second).get_trace_half_width(0), 77);
        assert_eq!(rules.net_classes.count(), 2);
    }

    #[test]
    fn append_net_class_copies_the_default_class_settings() {
        let mut rules = rules();
        rules.get_default_net_class();
        let appended = rules.append_net_class();
        assert_eq!(rules.net_classes.get(appended).get_name(), "class1");
        assert_eq!(
            rules.net_classes.get(appended).get_trace_half_width(0),
            1500
        );
        assert_eq!(
            rules.net_classes.get(appended).get_trace_clearance_class(),
            1
        );
    }

    #[test]
    fn create_default_via_rule_keeps_the_smallest_pad_per_layer_range() {
        let mut rules = rules();
        let default_class = rules.get_default_net_class();
        rules
            .via_infos
            .add(ViaInfo::new("a", PadstackId(0), 1, false));
        rules
            .via_infos
            .add(ViaInfo::new("b", PadstackId(1), 1, false));
        rules
            .via_infos
            .add(ViaInfo::new("c", PadstackId(2), 1, false));
        rules
            .via_infos
            .add(ViaInfo::new("d", PadstackId(3), 2, false));

        rules.create_default_via_rule(default_class, "default", &TestPadstacks);

        assert_eq!(rules.via_rules.len(), 1);
        let rule = &rules.via_rules[0];
        assert_eq!(rule.name, "default");
        assert_eq!(rule.via_count(), 2);
        assert!(rule.contains(rules.via_infos.get(ViaInfoId(1))));
        assert!(rule.contains(rules.via_infos.get(ViaInfoId(2))));
        assert!(!rule.contains(rules.via_infos.get(ViaInfoId(0))));
        assert_eq!(
            rules.net_classes.get(default_class).get_via_rule(),
            Some(&rules.via_rules[0])
        );
        assert_eq!(rules.get_default_via_rule(), Some(&rules.via_rules[0]));
        assert_eq!(rules.get_default_via_rule_id(), Some(ViaRuleId(0)));
        assert_eq!(rules.get_via_rule("default"), Some(ViaRuleId(0)));
        assert_eq!(rules.get_via_rule("nope"), None);
    }

    #[test]
    fn create_default_via_rule_does_nothing_without_via_infos() {
        let mut rules = rules();
        let default_class = rules.get_default_net_class();
        rules.create_default_via_rule(default_class, "default", &TestPadstacks);
        assert!(rules.via_rules.is_empty());
        assert_eq!(rules.net_classes.get(default_class).get_via_rule(), None);
    }

    #[test]
    fn default_via_diameter() {
        let mut rules = rules();
        assert_eq!(rules.get_default_via_diameter(&TestPadstacks), 0.0);

        let default_class = rules.get_default_net_class();
        rules
            .via_infos
            .add(ViaInfo::new("a", PadstackId(3), 1, false));
        rules.create_default_via_rule(default_class, "default", &TestPadstacks);
        assert_eq!(rules.get_default_via_diameter(&TestPadstacks), 70.0);

        rules.via_rules[0] = ViaRule::empty();
        assert_eq!(rules.get_default_via_diameter(&TestPadstacks), 0.0);
    }

    #[test]
    fn change_clearance_class_index_renumbers_items_net_classes_and_vias() {
        let mut rules = rules();
        rules.clearance_matrix.append_class("power");
        let default_class = rules.get_default_net_class();
        rules
            .net_classes
            .get_mut(default_class)
            .set_trace_clearance_class(1);
        rules
            .via_infos
            .add(ViaInfo::new("a", PadstackId(0), 1, false));
        let mut items = [
            TestItem {
                clearance_class_index: 1,
            },
            TestItem {
                clearance_class_index: 2,
            },
        ];

        rules.change_clearance_class_index(1, 2, items.iter_mut());

        assert_eq!(items[0].clearance_class_index, 2);
        assert_eq!(items[1].clearance_class_index, 2);
        assert_eq!(
            rules
                .net_classes
                .get(default_class)
                .get_trace_clearance_class(),
            2
        );
        assert_eq!(
            rules
                .net_classes
                .get(default_class)
                .default_item_clearance_classes
                .get(ItemClass::Via),
            2
        );
        assert_eq!(
            rules
                .via_infos
                .get(ViaInfoId(0))
                .get_clearance_class_index(),
            2
        );
    }

    #[test]
    fn clearance_class_renumbering_does_not_reach_a_rules_copy() {
        let mut rules = rules();
        rules.clearance_matrix.append_class("power");
        rules
            .via_infos
            .add(ViaInfo::new("a", PadstackId(0), 1, false));
        let mut rule = ViaRule::new("r");
        rule.append_via(rules.via_infos.get(ViaInfoId(0)).clone());
        rules.via_rules.push(rule);

        let mut items: Vec<TestItem> = Vec::new();
        rules.change_clearance_class_index(1, 2, items.iter_mut());

        assert_eq!(
            rules
                .via_infos
                .get(ViaInfoId(0))
                .get_clearance_class_index(),
            2,
            "the list entry is renumbered, as in Java"
        );
        assert_eq!(
            rules.via_rules[0].get_via(0).get_clearance_class_index(),
            1,
            "the rule's copy is not — Java's rule, sharing the object, would answer 2"
        );
    }

    #[test]
    fn remove_clearance_class_refuses_while_the_class_is_in_use() {
        let mut rules = rules();
        rules.clearance_matrix.append_class("power");
        let default_class = rules.get_default_net_class();
        assert_eq!(rules.clearance_matrix.get_class_count(), 3);

        let mut items = [TestItem {
            clearance_class_index: 2,
        }];
        assert!(!rules.remove_clearance_class(2, items.iter_mut()));
        assert_eq!(rules.clearance_matrix.get_class_count(), 3);

        let mut items: Vec<TestItem> = Vec::new();
        assert!(!rules.remove_clearance_class(1, items.iter_mut()));

        rules
            .via_infos
            .add(ViaInfo::new("a", PadstackId(0), 2, false));
        assert!(!rules.remove_clearance_class(2, items.iter_mut()));

        rules
            .via_infos
            .get_mut(ViaInfoId(0))
            .set_clearance_class_index(1);
        rules
            .net_classes
            .get_mut(default_class)
            .default_item_clearance_classes
            .set_all(1);
        let mut items = [TestItem {
            clearance_class_index: 3,
        }];
        assert!(rules.remove_clearance_class(2, items.iter_mut()));
        assert_eq!(rules.clearance_matrix.get_class_count(), 2);
        assert_eq!(items[0].clearance_class_index, 2);
    }

    #[test]
    fn accessor_round_trips() {
        let mut rules = rules();
        rules.set_ignore_conduction(false);
        assert!(!rules.get_ignore_conduction());
        rules.set_pin_edge_to_turn_dist(3.5);
        assert_eq!(rules.get_pin_edge_to_turn_dist(), 3.5);
        rules.set_use_slow_autoroute_algorithm(true);
        assert!(rules.get_use_slow_autoroute_algorithm());
        rules.trace_angle_restriction = AngleRestriction::NinetyDegree;
        assert_eq!(
            rules.trace_angle_restriction,
            AngleRestriction::NinetyDegree
        );
        assert_eq!(rules.layer_structure().count(), 2);
    }
}
