use std::fmt;

use crate::ids::NetClassId;
use crate::rules::ViaRule;
use crate::structure::LayerStructure;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ItemClass {
    None,
    Trace,
    Via,
    Pin,
    Smd,
    Area,
}

impl ItemClass {
    pub const VALUES: [ItemClass; 6] = [
        ItemClass::None,
        ItemClass::Trace,
        ItemClass::Via,
        ItemClass::Pin,
        ItemClass::Smd,
        ItemClass::Area,
    ];

    pub fn ordinal(self) -> usize {
        match self {
            ItemClass::None => 0,
            ItemClass::Trace => 1,
            ItemClass::Via => 2,
            ItemClass::Pin => 3,
            ItemClass::Smd => 4,
            ItemClass::Area => 5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DefaultItemClearanceClasses {
    clearance_classes: [usize; ItemClass::VALUES.len()],
}

impl Default for DefaultItemClearanceClasses {
    fn default() -> Self {
        DefaultItemClearanceClasses::new()
    }
}

impl DefaultItemClearanceClasses {
    pub fn new() -> DefaultItemClearanceClasses {
        let mut result = DefaultItemClearanceClasses {
            clearance_classes: [0; ItemClass::VALUES.len()],
        };
        result.set_all(1);
        result
    }

    pub fn get(&self, item_class: ItemClass) -> usize {
        self.clearance_classes[item_class.ordinal()]
    }

    pub fn set(&mut self, item_class: ItemClass, index: usize) {
        self.clearance_classes[item_class.ordinal()] = index;
    }

    pub fn set_all(&mut self, index: usize) {
        for i in 1..self.clearance_classes.len() {
            self.clearance_classes[i] = index;
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NetClass {
    name: String,
    trace_half_width: Vec<i32>,
    active_routing_layer: Vec<bool>,
    pub default_item_clearance_classes: DefaultItemClearanceClasses,
    pub is_ignored_by_autorouter: bool,
    via_rule: Option<ViaRule>,
    trace_clearance_class: usize,
    shove_fixed: bool,
    pull_tight: bool,
    ignore_cycles_with_areas: bool,
    minimum_trace_length: f64,
    maximum_trace_length: f64,
}

impl NetClass {
    pub fn new(
        name: impl Into<String>,
        layer_structure: &LayerStructure,
        ignored_by_autorouter: bool,
    ) -> NetClass {
        NetClass {
            name: name.into(),
            trace_half_width: vec![0; layer_structure.count()],
            active_routing_layer: layer_structure.layers.iter().map(|l| l.is_signal).collect(),
            default_item_clearance_classes: DefaultItemClearanceClasses::new(),
            is_ignored_by_autorouter: ignored_by_autorouter,
            via_rule: None,
            trace_clearance_class: 0,
            shove_fixed: false,
            pull_tight: true,
            ignore_cycles_with_areas: false,
            minimum_trace_length: 0.0,
            maximum_trace_length: 0.0,
        }
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }

    pub fn set_trace_half_width_on_all_layers(&mut self, value: i32) {
        self.trace_half_width.fill(value);
    }

    pub fn set_trace_half_width(&mut self, layer: usize, value: i32) {
        self.trace_half_width[layer] = value;
    }

    pub fn set_trace_half_width_on_inner(&mut self, value: i32) {
        for i in 1..self.trace_half_width.len().saturating_sub(1) {
            self.trace_half_width[i] = value;
        }
    }

    pub fn layer_count(&self) -> usize {
        self.trace_half_width.len()
    }

    pub fn get_trace_half_width(&self, layer: usize) -> i32 {
        self.trace_half_width.get(layer).copied().unwrap_or(0)
    }

    pub fn get_trace_clearance_class(&self) -> usize {
        self.trace_clearance_class
    }

    pub fn set_trace_clearance_class(&mut self, clearance_class: usize) {
        self.trace_clearance_class = clearance_class;
    }

    pub fn get_via_rule(&self) -> Option<&ViaRule> {
        self.via_rule.as_ref()
    }

    pub fn set_via_rule(&mut self, via_rule: Option<ViaRule>) {
        self.via_rule = via_rule;
    }

    pub fn is_shove_fixed(&self) -> bool {
        self.shove_fixed
    }

    pub fn set_shove_fixed(&mut self, value: bool) {
        self.shove_fixed = value;
    }

    pub fn get_pull_tight(&self) -> bool {
        self.pull_tight
    }

    pub fn set_pull_tight(&mut self, value: bool) {
        self.pull_tight = value;
    }

    pub fn get_ignore_cycles_with_areas(&self) -> bool {
        self.ignore_cycles_with_areas
    }

    pub fn set_ignore_cycles_with_areas(&mut self, value: bool) {
        self.ignore_cycles_with_areas = value;
    }

    pub fn get_minimum_trace_length(&self) -> f64 {
        self.minimum_trace_length
    }

    pub fn set_minimum_trace_length(&mut self, value: f64) {
        self.minimum_trace_length = value;
    }

    pub fn get_maximum_trace_length(&self) -> f64 {
        self.maximum_trace_length
    }

    pub fn set_maximum_trace_length(&mut self, value: f64) {
        self.maximum_trace_length = value;
    }

    pub fn is_active_routing_layer(&self, layer_number: usize) -> bool {
        self.active_routing_layer
            .get(layer_number)
            .copied()
            .unwrap_or(false)
    }

    pub fn set_active_routing_layer(&mut self, layer_number: usize, active: bool) {
        if let Some(slot) = self.active_routing_layer.get_mut(layer_number) {
            *slot = active;
        }
    }

    pub fn set_all_layers_active(&mut self, value: bool) {
        self.active_routing_layer.fill(value);
    }

    pub fn set_all_inner_layers_active(&mut self, value: bool) {
        for i in 1..self.active_routing_layer.len().saturating_sub(1) {
            self.active_routing_layer[i] = value;
        }
    }

    pub fn trace_width_is_layer_dependent(&self, layer_structure: &LayerStructure) -> bool {
        let compare_value = self.trace_half_width[0];
        (1..self.trace_half_width.len()).any(|i| {
            layer_structure.layers[i].is_signal && self.trace_half_width[i] != compare_value
        })
    }

    pub fn trace_width_is_inner_layer_dependent(&self, layer_structure: &LayerStructure) -> bool {
        if self.trace_half_width.len() <= 3 {
            return false;
        }
        let mut first_inner_layer_no = 1usize;
        while !layer_structure.layers[first_inner_layer_no].is_signal {
            first_inner_layer_no += 1;
        }
        if first_inner_layer_no >= self.trace_half_width.len() - 1 {
            return false;
        }
        let compare_width = self.trace_half_width[first_inner_layer_no];
        (first_inner_layer_no + 1..self.trace_half_width.len() - 1).any(|i| {
            layer_structure.layers[i].is_signal && self.trace_half_width[i] != compare_width
        })
    }
}

impl fmt::Display for NetClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct NetClasses {
    classes: Vec<NetClass>,
}

impl NetClasses {
    pub fn new() -> NetClasses {
        NetClasses::default()
    }

    pub fn count(&self) -> usize {
        self.classes.len()
    }

    pub fn get(&self, index: NetClassId) -> &NetClass {
        &self.classes[index.0]
    }

    pub fn get_mut(&mut self, index: NetClassId) -> &mut NetClass {
        &mut self.classes[index.0]
    }

    pub fn get_by_name(&self, name: &str) -> Option<&NetClass> {
        self.get_no(name).map(|id| self.get(id))
    }

    pub fn get_no(&self, name: &str) -> Option<NetClassId> {
        self.classes
            .iter()
            .position(|c| c.name == name)
            .map(NetClassId)
    }

    pub fn iter(&self) -> std::slice::Iter<'_, NetClass> {
        self.classes.iter()
    }

    pub fn append(
        &mut self,
        name: impl Into<String>,
        layer_structure: &LayerStructure,
        ignored_by_autorouter: bool,
    ) -> NetClassId {
        self.classes
            .push(NetClass::new(name, layer_structure, ignored_by_autorouter));
        NetClassId(self.classes.len() - 1)
    }

    pub fn append_with_generated_name(&mut self, layer_structure: &LayerStructure) -> NetClassId {
        let mut index = 0;
        let new_name = loop {
            index += 1;
            let candidate = format!("class{index}");
            if self.get_no(&candidate).is_none() {
                break candidate;
            }
        };
        self.append(new_name, layer_structure, false)
    }

    pub fn find(
        &self,
        trace_half_width: i32,
        trace_clearance_class: usize,
        via_rule: Option<&ViaRule>,
    ) -> Option<NetClassId> {
        self.classes
            .iter()
            .position(|c| {
                c.trace_clearance_class == trace_clearance_class
                    && c.via_rule.as_ref() == via_rule
                    && (0..c.layer_count()).all(|i| c.get_trace_half_width(i) == trace_half_width)
            })
            .map(NetClassId)
    }

    pub fn find_per_layer(
        &self,
        trace_half_width: &[i32],
        trace_clearance_class: usize,
        via_rule: Option<&ViaRule>,
    ) -> Option<NetClassId> {
        self.classes
            .iter()
            .position(|c| {
                c.trace_clearance_class == trace_clearance_class
                    && c.via_rule.as_ref() == via_rule
                    && trace_half_width.len() == c.layer_count()
                    && (0..c.layer_count())
                        .all(|i| c.get_trace_half_width(i) == trace_half_width[i])
            })
            .map(NetClassId)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structure::Layer;

    fn layers() -> LayerStructure {
        LayerStructure::new(vec![
            Layer::new("Top", true),
            Layer::new("Inner1", false),
            Layer::new("Inner2", true),
            Layer::new("Bottom", true),
        ])
    }

    #[test]
    fn default_item_clearance_classes_leave_none_at_zero() {
        let classes = DefaultItemClearanceClasses::new();
        assert_eq!(classes.get(ItemClass::None), 0);
        for item_class in ItemClass::VALUES.iter().skip(1) {
            assert_eq!(classes.get(*item_class), 1);
        }
    }

    #[test]
    fn set_all_still_skips_none_but_set_does_not() {
        let mut classes = DefaultItemClearanceClasses::new();
        classes.set_all(4);
        assert_eq!(classes.get(ItemClass::None), 0);
        assert_eq!(classes.get(ItemClass::Via), 4);
        classes.set(ItemClass::None, 7);
        assert_eq!(classes.get(ItemClass::None), 7);
    }

    #[test]
    fn item_class_ordinals_match_java_declaration_order() {
        assert_eq!(ItemClass::None.ordinal(), 0);
        assert_eq!(ItemClass::Trace.ordinal(), 1);
        assert_eq!(ItemClass::Via.ordinal(), 2);
        assert_eq!(ItemClass::Pin.ordinal(), 3);
        assert_eq!(ItemClass::Smd.ordinal(), 4);
        assert_eq!(ItemClass::Area.ordinal(), 5);
    }

    #[test]
    fn new_net_class_activates_exactly_the_signal_layers() {
        let net_class = NetClass::new("default", &layers(), false);
        assert!(net_class.is_active_routing_layer(0));
        assert!(!net_class.is_active_routing_layer(1));
        assert!(net_class.is_active_routing_layer(2));
        assert!(net_class.is_active_routing_layer(3));
        assert!(!net_class.is_active_routing_layer(4));
        assert!(net_class.get_pull_tight());
        assert!(!net_class.is_shove_fixed());
        assert_eq!(net_class.get_trace_clearance_class(), 0);
        assert_eq!(net_class.get_via_rule(), None);
        assert_eq!(net_class.layer_count(), 4);
    }

    #[test]
    fn trace_half_width_setters_cover_all_inner_and_single_layers() {
        let mut net_class = NetClass::new("default", &layers(), false);
        net_class.set_trace_half_width_on_all_layers(100);
        assert_eq!(
            (0..4)
                .map(|i| net_class.get_trace_half_width(i))
                .collect::<Vec<_>>(),
            vec![100, 100, 100, 100]
        );

        net_class.set_trace_half_width_on_inner(50);
        assert_eq!(
            (0..4)
                .map(|i| net_class.get_trace_half_width(i))
                .collect::<Vec<_>>(),
            vec![100, 50, 50, 100]
        );

        net_class.set_trace_half_width(3, 7);
        assert_eq!(net_class.get_trace_half_width(3), 7);
        assert_eq!(net_class.get_trace_half_width(9), 0);
    }

    #[test]
    fn active_routing_layer_setters() {
        let mut net_class = NetClass::new("default", &layers(), false);
        net_class.set_all_layers_active(false);
        assert!((0..4).all(|i| !net_class.is_active_routing_layer(i)));
        net_class.set_all_inner_layers_active(true);
        assert!(!net_class.is_active_routing_layer(0));
        assert!(net_class.is_active_routing_layer(1));
        assert!(net_class.is_active_routing_layer(2));
        assert!(!net_class.is_active_routing_layer(3));
        net_class.set_active_routing_layer(99, true);
        assert!(!net_class.is_active_routing_layer(99));
    }

    #[test]
    fn trace_width_is_layer_dependent_skips_non_signal_layers() {
        let layers = layers();
        let mut net_class = NetClass::new("default", &layers, false);
        net_class.set_trace_half_width_on_all_layers(100);
        assert!(!net_class.trace_width_is_layer_dependent(&layers));

        net_class.set_trace_half_width(1, 999);
        assert!(!net_class.trace_width_is_layer_dependent(&layers));

        net_class.set_trace_half_width(2, 200);
        assert!(net_class.trace_width_is_layer_dependent(&layers));
    }

    #[test]
    fn trace_width_is_inner_layer_dependent() {
        let three = LayerStructure::new(vec![
            Layer::new("Top", true),
            Layer::new("Mid", true),
            Layer::new("Bottom", true),
        ]);
        let net_class = NetClass::new("default", &three, false);
        assert!(!net_class.trace_width_is_inner_layer_dependent(&three));

        let layers = layers();
        let mut net_class = NetClass::new("default", &layers, false);
        net_class.set_trace_half_width_on_all_layers(100);
        assert!(!net_class.trace_width_is_inner_layer_dependent(&layers));
        net_class.set_trace_half_width(2, 200);
        assert!(!net_class.trace_width_is_inner_layer_dependent(&layers));

        let five = LayerStructure::new(vec![
            Layer::new("Top", true),
            Layer::new("In1", true),
            Layer::new("In2", true),
            Layer::new("In3", true),
            Layer::new("Bottom", true),
        ]);
        let mut net_class = NetClass::new("default", &five, false);
        net_class.set_trace_half_width_on_all_layers(100);
        assert!(!net_class.trace_width_is_inner_layer_dependent(&five));
        net_class.set_trace_half_width(2, 200);
        assert!(net_class.trace_width_is_inner_layer_dependent(&five));
    }

    #[test]
    #[should_panic(expected = "index out of bounds")]
    fn trace_width_is_inner_layer_dependent_reproduces_the_java_scan_overrun() {
        let layers = LayerStructure::new(vec![
            Layer::new("Top", true),
            Layer::new("In1", false),
            Layer::new("In2", false),
            Layer::new("Bottom", false),
        ]);
        let net_class = NetClass::new("default", &layers, false);
        let _ = net_class.trace_width_is_inner_layer_dependent(&layers);
    }

    #[test]
    fn net_classes_append_and_lookup() {
        let layers = layers();
        let mut net_classes = NetClasses::new();
        assert_eq!(net_classes.count(), 0);

        let default = net_classes.append("default", &layers, false);
        assert_eq!(default, NetClassId(0));
        assert_eq!(net_classes.get(default).get_name(), "default");
        assert_eq!(net_classes.get_no("default"), Some(NetClassId(0)));
        assert_eq!(net_classes.get_no("DEFAULT"), None);
        assert!(net_classes.get_by_name("nope").is_none());
    }

    #[test]
    fn append_with_generated_name_skips_taken_names() {
        let layers = layers();
        let mut net_classes = NetClasses::new();
        net_classes.append("class1", &layers, false);
        let generated = net_classes.append_with_generated_name(&layers);
        assert_eq!(net_classes.get(generated).get_name(), "class2");
        let generated = net_classes.append_with_generated_name(&layers);
        assert_eq!(net_classes.get(generated).get_name(), "class3");
    }

    #[test]
    fn find_matches_uniform_width_clearance_class_and_via_rule() {
        let layers = layers();
        let mut net_classes = NetClasses::new();
        let id = net_classes.append("default", &layers, false);
        net_classes
            .get_mut(id)
            .set_trace_half_width_on_all_layers(150);
        net_classes.get_mut(id).set_trace_clearance_class(1);
        let rule = ViaRule::new("default");
        let other = ViaRule::new("other");
        net_classes.get_mut(id).set_via_rule(Some(rule.clone()));

        assert_eq!(net_classes.find(150, 1, Some(&rule)), Some(id));
        assert_eq!(net_classes.find(150, 2, Some(&rule)), None);
        assert_eq!(net_classes.find(150, 1, None), None);
        assert_eq!(net_classes.find(150, 1, Some(&other)), None);
        assert_eq!(net_classes.find(151, 1, Some(&rule)), None);

        assert_eq!(
            net_classes.find_per_layer(&[150, 150, 150, 150], 1, Some(&rule)),
            Some(id)
        );
        assert_eq!(
            net_classes.find_per_layer(&[150, 150, 150], 1, Some(&rule)),
            None
        );
        net_classes.get_mut(id).set_trace_half_width(2, 90);
        assert_eq!(
            net_classes.find_per_layer(&[150, 150, 90, 150], 1, Some(&rule)),
            Some(id)
        );
        assert_eq!(net_classes.find(150, 1, Some(&rule)), None);
    }
}
