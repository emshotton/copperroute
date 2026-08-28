//! `BoardRules`: everything an item must satisfy to be inserted into a routing board.
//!
//! Java: `rules/BoardRules.java`.

use crate::ids::{NetClassId, ViaInfoId, ViaRuleId};
use crate::structure::{AngleRestriction, LayerStructure};

use super::{
    ClearanceClassIndexed, ClearanceMatrix, ItemClass, NetClasses, Nets, PadstackLookup, ViaInfos,
    ViaRule,
};

/// Port of `BoardRules` (`rules/BoardRules.java`): the rules and constraints required for items
/// to be inserted into a routing board.
///
/// not ported: `BoardRules.writeObject`/`readObject` (BoardRules.java:424-434). Both are private
/// Java-serialization hooks whose whole job is to write and read back the `transient`
/// `traceAngleRestriction` field; this port has no Java-serialized state.
///
/// not ported: `BasicBoard.clearanceValue(class1, class2, layer)`
/// (`board/facade/BasicBoard.java:1110-1115`) is *not* a `BoardRules` method, despite the Task 2
/// brief listing it here. It is a board-level wrapper around
/// `rules.clearanceMatrix.getValue(class1, class2, layer, true)`.
// added in Task 11: `Board::clearance_value`, which is that wrapper (including the
// `rules == null` guard's `return 0`, which cannot arise in this port).
#[derive(Debug, Clone, PartialEq)]
pub struct BoardRules {
    /// `BoardRules.clearanceMatrix` (BoardRules.java:20); public and final in Java too.
    pub clearance_matrix: ClearanceMatrix,
    /// `BoardRules.nets` (BoardRules.java:23); public and final in Java too.
    pub nets: Nets,
    /// `BoardRules.viaInfos` (BoardRules.java:25); public and final in Java too.
    pub via_infos: ViaInfos,
    /// `BoardRules.viaRules` (BoardRules.java:26), a `Vector<ViaRule>`; public and final in Java
    /// too. Indices into it are [`ViaRuleId`]s.
    pub via_rules: Vec<ViaRule>,
    /// `BoardRules.netClasses` (BoardRules.java:27); public and final in Java too.
    pub net_classes: NetClasses,
    /// `BoardRules.layerStructure` (BoardRules.java:28). Java holds a reference to the board's
    /// stack; this port owns a copy, which is equivalent because a board's layer stack is fixed
    /// at construction.
    layer_structure: LayerStructure,
    /// `BoardRules.traceAngleRestriction` (BoardRules.java:31), defaulting to
    /// `FORTYFIVE_DEGREE` (BoardRules.java:56).
    ///
    /// Java marks the field `transient` *not* because it is scratch state but because
    /// `AngleRestriction` is a Java enum that the surrounding serialization format could not
    /// round-trip safely; the two private hooks at BoardRules.java:424-434 write and read it back
    /// by ordinal by hand. With no Java serialization here, it is an ordinary public field.
    pub trace_angle_restriction: AngleRestriction,
    /// `BoardRules.ignoreConduction` (BoardRules.java:34), which Java initialises to `true`.
    ignore_conduction: bool,
    /// `BoardRules.minTraceHalfWidth` (BoardRules.java:37), seeded to 100000
    /// (BoardRules.java:58).
    min_trace_half_width: i32,
    /// `BoardRules.maxTraceHalfWidth` (BoardRules.java:40), seeded to 100 (BoardRules.java:59).
    max_trace_half_width: i32,
    /// `BoardRules.pinEdgeToTurnDist` (BoardRules.java:46); `<= 0` means "no exit restrictions".
    pin_edge_to_turn_dist: f64,
    /// `BoardRules.useSlowAutorouteAlgorithm` (BoardRules.java:48).
    use_slow_autoroute_algorithm: bool,
    /// `BoardRules.holeClearance` (BoardRules.java:49).
    hole_clearance: i32,
}

impl BoardRules {
    /// Port of the `BoardRules(LayerStructure, ClearanceMatrix)` constructor
    /// (BoardRules.java:52-61).
    ///
    /// Note the deliberately inverted seeds at BoardRules.java:58-59: `minTraceHalfWidth` starts
    /// at 100000 and `maxTraceHalfWidth` at 100, so that the first
    /// [`Self::set_default_trace_half_width`] call pulls both towards the real value. Until such
    /// a call happens, `min > max`.
    pub fn new(layer_structure: LayerStructure, clearance_matrix: ClearanceMatrix) -> BoardRules {
        BoardRules {
            clearance_matrix,
            nets: Nets::new(),
            via_infos: ViaInfos::new(),
            via_rules: Vec::new(),
            net_classes: NetClasses::new(),
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

    /// The board's layer stack. Not a Java accessor — `BoardRules.layerStructure` is private and
    /// used only inside the class; this port needs it for the [`super::NetClass`] methods that no
    /// longer hold a layer structure of their own.
    pub fn layer_structure(&self) -> &LayerStructure {
        &self.layer_structure
    }

    /// Port of `BoardRules.defaultClearanceClass` (BoardRules.java:64-66): the constant 1.
    pub fn default_clearance_class() -> usize {
        1
    }

    /// Port of `BoardRules.clearanceClassNone` (BoardRules.java:69-71): the constant 0, the
    /// clearance class used for items with no clearances.
    pub fn clearance_class_none() -> usize {
        0
    }

    /// Port of `BoardRules.getTraceHalfWidth` (BoardRules.java:74-77): the trace half width used
    /// for routing the given net on the given layer.
    ///
    /// Java dereferences `nets.get(netNumber)` without a null check, so an unknown net number is
    /// a `NullPointerException`; the port panics with the same meaning.
    pub fn get_trace_half_width(&self, net_number: i32, layer: usize) -> i32 {
        let net = self
            .nets
            .get(net_number)
            .expect("BoardRules.getTraceHalfWidth: unknown net (Java NPEs at BoardRules.java:76)");
        self.net_classes
            .get(net.net_class)
            .get_trace_half_width(layer)
    }

    /// Port of `BoardRules.traceWidthsAreLayerDependent` (BoardRules.java:83-91).
    ///
    /// Java's doc claims that `netNumber < 0` checks "the default trace widths for all nets", but
    /// the body only calls [`Self::get_trace_half_width`], which for any number outside
    /// `1 ..= nets.size()` dereferences `null`. The doc is simply wrong; the port panics for the
    /// same inputs. The method has no caller anywhere in the Java tree.
    pub fn trace_widths_are_layer_dependent(&self, net_number: i32) -> bool {
        let compare_width = self.get_trace_half_width(net_number, 0);
        (1..self.layer_structure.count())
            .any(|i| self.get_trace_half_width(net_number, i) != compare_width)
    }

    /// Port of `BoardRules.getMinTraceHalfWidth` (BoardRules.java:94-96).
    pub fn get_min_trace_half_width(&self) -> i32 {
        self.min_trace_half_width
    }

    /// Port of `BoardRules.getMaxTraceHalfWidth` (BoardRules.java:99-101).
    pub fn get_max_trace_half_width(&self) -> i32 {
        self.max_trace_half_width
    }

    /// Port of `BoardRules.getHoleClearance` (BoardRules.java:104-106).
    pub fn get_hole_clearance(&self) -> i32 {
        self.hole_clearance
    }

    /// Port of `BoardRules.setHoleClearance` (BoardRules.java:109-111): clamps to `>= 0`.
    pub fn set_hole_clearance(&mut self, value: i32) {
        self.hole_clearance = value.max(0);
    }

    /// Port of `BoardRules.setDefaultTraceHalfWidth(int, int)` (BoardRules.java:114-118): sets
    /// the default net class's half width on one layer and folds the value into the running
    /// min/max.
    pub fn set_default_trace_half_width(&mut self, layer: usize, value: i32) {
        let default_class = self.get_default_net_class();
        self.net_classes
            .get_mut(default_class)
            .set_trace_half_width(layer, value);
        self.min_trace_half_width = self.min_trace_half_width.min(value);
        self.max_trace_half_width = self.max_trace_half_width.max(value);
    }

    /// Port of `BoardRules.getDefaultTraceHalfWidth` (BoardRules.java:121-123).
    pub fn get_default_trace_half_width(&mut self, layer: usize) -> i32 {
        let default_class = self.get_default_net_class();
        self.net_classes
            .get(default_class)
            .get_trace_half_width(layer)
    }

    /// Port of `BoardRules.setDefaultTraceHalfWidths` (BoardRules.java:126-134): sets the default
    /// net class's half width on *all* layers.
    ///
    /// A `value <= 0` is rejected outright (Java warns and returns without touching anything).
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

    /// Port of `BoardRules.getDefaultNetClass` (BoardRules.java:137-143): net class 0, creating
    /// it first if the net-class list is still empty.
    ///
    /// Java's getter mutates, so this one takes `&mut self`.
    pub fn get_default_net_class(&mut self) -> NetClassId {
        if self.net_classes.count() == 0 {
            self.create_default_net_class();
        }
        NetClassId(0)
    }

    /// Port of `BoardRules.createDefaultNetClass` (BoardRules.java:202-209): appends the class
    /// named `"default"` with a trace half width of 1500 and trace clearance class 1.
    pub fn create_default_net_class(&mut self) {
        let id = self
            .net_classes
            .append("default", &self.layer_structure, false);
        let default_trace_half_width = 1500;
        let net_class = self.net_classes.get_mut(id);
        net_class.set_trace_half_width_on_all_layers(default_trace_half_width);
        net_class.set_trace_clearance_class(1);
    }

    /// Port of `BoardRules.getNewNetClass()` (BoardRules.java:146-152): a new net class with a
    /// generated name, seeded from the default class's clearance class, via rule and layer-0
    /// trace half width.
    pub fn get_new_net_class(&mut self) -> NetClassId {
        let result = self
            .net_classes
            .append_with_generated_name(&self.layer_structure);
        self.initialize_new_net_class(result);
        result
    }

    /// Port of `BoardRules.getNewNetClass(String)` (BoardRules.java:155-162).
    pub fn get_new_net_class_named(&mut self, name: impl Into<String>) -> NetClassId {
        let result = self.net_classes.append(name, &self.layer_structure, false);
        self.initialize_new_net_class(result);
        result
    }

    /// The three-line tail shared by both `getNewNetClass` overloads
    /// (BoardRules.java:148-150 and :158-160).
    fn initialize_new_net_class(&mut self, result: NetClassId) {
        let default_class = self.get_default_net_class();
        let trace_clearance_class = self
            .net_classes
            .get(default_class)
            .get_trace_clearance_class();
        let trace_half_width = self.net_classes.get(default_class).get_trace_half_width(0);
        let default_via_rule = self.get_default_via_rule();
        let net_class = self.net_classes.get_mut(result);
        net_class.set_trace_clearance_class(trace_clearance_class);
        net_class.set_via_rule(default_via_rule);
        net_class.set_trace_half_width_on_all_layers(trace_half_width);
    }

    /// Port of `BoardRules.appendNetClass()` (BoardRules.java:212-219): a new net class with a
    /// generated name, seeded from net class 0.
    ///
    /// Note the difference from [`Self::get_new_net_class`]: this one reads
    /// `netClasses.get(0)` directly rather than through `getDefaultNetClass()`, so it does *not*
    /// create the default class first and panics on an empty list exactly where Java's
    /// `Vector.get(0)` throws.
    pub fn append_net_class(&mut self) -> NetClassId {
        let new_class = self
            .net_classes
            .append_with_generated_name(&self.layer_structure);
        let default_class = self.net_classes.get(NetClassId(0));
        let via_rule = default_class.get_via_rule();
        let trace_half_width = default_class.get_trace_half_width(0);
        let trace_clearance_class = default_class.get_trace_clearance_class();
        let new = self.net_classes.get_mut(new_class);
        new.set_via_rule(via_rule);
        new.set_trace_half_width_on_all_layers(trace_half_width);
        new.set_trace_clearance_class(trace_clearance_class);
        new_class
    }

    /// Port of `BoardRules.appendNetClass(String)` (BoardRules.java:225-239): as
    /// [`Self::append_net_class`], but named, returning the existing class untouched if the name
    /// is already taken, and additionally copying the default class's
    /// [`super::DefaultItemClearanceClasses`].
    pub fn append_net_class_named(&mut self, name: &str) -> NetClassId {
        if let Some(found) = self.net_classes.get_no(name) {
            return found;
        }
        let new_class = self.net_classes.append(name, &self.layer_structure, false);
        let default_class = self.net_classes.get(NetClassId(0));
        let default_item_clearance_classes = default_class.default_item_clearance_classes;
        let via_rule = default_class.get_via_rule();
        let trace_half_width = default_class.get_trace_half_width(0);
        let trace_clearance_class = default_class.get_trace_clearance_class();
        let new = self.net_classes.get_mut(new_class);
        new.default_item_clearance_classes = default_item_clearance_classes;
        new.set_via_rule(via_rule);
        new.set_trace_half_width_on_all_layers(trace_half_width);
        new.set_trace_clearance_class(trace_clearance_class);
        new_class
    }

    /// Port of `BoardRules.createDefaultViaRule` (BoardRules.java:169-199): builds a via rule
    /// named `name` holding every via info whose clearance class matches the net class's default
    /// via clearance class, keeping only the smallest pad per layer range, appends it to
    /// [`Self::via_rules`] and assigns it to `net_class`.
    ///
    /// Does nothing at all when there are no via infos (BoardRules.java:170-172) — not even
    /// appending an empty rule.
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
            let current_via_info = ViaInfoId(i);
            let info = self.via_infos.get(current_via_info);
            if info.get_clearance_class_index() != default_via_cl_class {
                continue;
            }
            let current_padstack = info.get_padstack();
            let current_from_layer = padstacks.padstack_from_layer(current_padstack);
            let current_to_layer = padstacks.padstack_to_layer(current_padstack);
            let existing_via = default_rule.get_layer_range(
                current_from_layer,
                current_to_layer,
                &self.via_infos,
                padstacks,
            );
            match existing_via {
                Some(existing) => {
                    // Java NPEs here if either padstack has no shape on `currentFromLayer`;
                    // both were selected by that same layer range, so it cannot happen.
                    let new_width = padstacks
                        .padstack_shape_max_width(current_padstack, current_from_layer)
                        .expect("padstack has a shape on its own fromLayer");
                    let existing_width = padstacks
                        .padstack_shape_max_width(
                            self.via_infos.get(existing).get_padstack(),
                            current_from_layer,
                        )
                        .expect("padstack has a shape on the matched fromLayer");
                    if new_width < existing_width {
                        // The via with the smallest pad shape is preferred.
                        default_rule.remove_via(existing);
                        default_rule.append_via(current_via_info);
                    }
                }
                None => default_rule.append_via(current_via_info),
            }
        }
        self.via_rules.push(default_rule);
        let new_rule_id = ViaRuleId(self.via_rules.len() - 1);
        self.net_classes
            .get_mut(net_class)
            .set_via_rule(Some(new_rule_id));
    }

    /// Port of `BoardRules.getDefaultViaRule` (BoardRules.java:242-247): the first via rule, or
    /// `None` (Java: `null`) when there is none.
    pub fn get_default_via_rule(&self) -> Option<ViaRuleId> {
        if self.via_rules.is_empty() {
            None
        } else {
            Some(ViaRuleId(0))
        }
    }

    /// Port of `BoardRules.getViaRule(String)` (BoardRules.java:250-257): the via rule with
    /// exactly this name (`equals`, case sensitive).
    pub fn get_via_rule(&self, name: &str) -> Option<ViaRuleId> {
        self.via_rules
            .iter()
            .position(|r| r.name == name)
            .map(ViaRuleId)
    }

    /// Port of `BoardRules.changeClearanceClassIndex` (BoardRules.java:263-289): renumbers
    /// clearance class `from_index` to `to_index` across board items, net classes (both the trace
    /// clearance class and every default item clearance class) and via infos.
    ///
    /// `board_items` is Java's `Collection<Item>` argument; see [`ClearanceClassIndexed`].
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

    /// Port of `BoardRules.removeClearanceClass` (BoardRules.java:295-349): removes clearance
    /// class `index`, returning false — and changing nothing — if any board item, net class or
    /// via info is still assigned to it.
    ///
    /// Once the class is free, every reference *above* `index` is decremented and the class is
    /// dropped from the matrix.
    ///
    /// `board_items` is Java's `Collection<Item>`, which Java iterates twice (the check pass and
    /// the renumber pass); the port collects the iterator so it can do the same.
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

    /// Port of `BoardRules.getPinEdgeToTurnDist` (BoardRules.java:356-358).
    pub fn get_pin_edge_to_turn_dist(&self) -> f64 {
        self.pin_edge_to_turn_dist
    }

    /// Port of `BoardRules.setPinEdgeToTurnDist` (BoardRules.java:365-367).
    pub fn set_pin_edge_to_turn_dist(&mut self, value: f64) {
        self.pin_edge_to_turn_dist = value;
    }

    /// Port of `BoardRules.getIgnoreConduction` (BoardRules.java:370-372).
    pub fn get_ignore_conduction(&self) -> bool {
        self.ignore_conduction
    }

    /// Port of `BoardRules.setIgnoreConduction` (BoardRules.java:375-377).
    pub fn set_ignore_conduction(&mut self, value: bool) {
        self.ignore_conduction = value;
    }

    /// Port of `BoardRules.getUseSlowAutorouteAlgorithm` (BoardRules.java:394-396).
    pub fn get_use_slow_autoroute_algorithm(&self) -> bool {
        self.use_slow_autoroute_algorithm
    }

    /// Port of `BoardRules.setUseSlowAutorouteAlgorithm` (BoardRules.java:403-405).
    pub fn set_use_slow_autoroute_algorithm(&mut self, value: bool) {
        self.use_slow_autoroute_algorithm = value;
    }

    /// Port of `BoardRules.getDefaultViaDiameter` (BoardRules.java:408-421): the maximum width of
    /// the default via rule's first via on its first and last layers, or 0 when there is no
    /// default rule or it holds no vias.
    ///
    /// Java NPEs if the padstack has no shape on its own `fromLayer()`/`toLayer()`, which can
    /// only happen for a padstack with no shapes at all; the port panics there.
    pub fn get_default_via_diameter(&self, padstacks: &impl PadstackLookup) -> f64 {
        let Some(default_via_rule) = self.get_default_via_rule() else {
            return 0.0;
        };
        let default_via_rule = &self.via_rules[default_via_rule.0];
        if default_via_rule.via_count() == 0 {
            return 0.0;
        }
        let via_padstack = self
            .via_infos
            .get(default_via_rule.get_via(0))
            .get_padstack();
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

// renamed: BoardRules.getTraceAngleRestriction (BoardRules.java:380-382) -> the public field
// renamed: BoardRules.setTraceAngleRestriction (BoardRules.java:385-387) -> the public field
// `BoardRules::trace_angle_restriction`. Both Java methods are one-line accessors over a field
// that is private only because it is `transient`; with no serialization the field is public.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::PadstackId;
    use crate::rules::ViaInfo;
    use crate::structure::Layer;

    /// A stand-in for Task 4's `Padstacks`: padstack `n` spans layers `n ..= n + 1` and has a
    /// max width of `100 - 10 * n`, so a higher index means a *smaller* pad.
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
        // BoardRules.java:56-60.
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
        // BoardRules.java:64-71.
        assert_eq!(BoardRules::default_clearance_class(), 1);
        assert_eq!(BoardRules::clearance_class_none(), 0);
    }

    #[test]
    fn get_default_net_class_creates_it_lazily() {
        // BoardRules.java:137-143 + :202-209.
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
        // Calling it again does not append a second class.
        assert_eq!(rules.get_default_net_class(), NetClassId(0));
        assert_eq!(rules.net_classes.count(), 1);
    }

    #[test]
    fn set_default_trace_half_width_updates_the_running_min_and_max() {
        // BoardRules.java:114-118: the seeds are min = 100000, max = 100.
        let mut rules = rules();
        rules.set_default_trace_half_width(0, 500);
        assert_eq!(rules.get_default_trace_half_width(0), 500);
        assert_eq!(rules.get_min_trace_half_width(), 500);
        assert_eq!(rules.get_max_trace_half_width(), 500);

        rules.set_default_trace_half_width(1, 900);
        assert_eq!(rules.get_min_trace_half_width(), 500);
        assert_eq!(rules.get_max_trace_half_width(), 900);
        // Layer 0 is untouched by a layer-1 write.
        assert_eq!(rules.get_default_trace_half_width(0), 500);
    }

    #[test]
    fn set_default_trace_half_widths_rejects_non_positive_values() {
        // BoardRules.java:127-130.
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
        // BoardRules.java:110.
        let mut rules = rules();
        rules.set_hole_clearance(42);
        assert_eq!(rules.get_hole_clearance(), 42);
        rules.set_hole_clearance(-1);
        assert_eq!(rules.get_hole_clearance(), 0);
    }

    #[test]
    fn get_trace_half_width_goes_through_the_nets_net_class() {
        // BoardRules.java:74-77.
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
    fn get_trace_half_width_panics_where_java_npes() {
        // BoardRules.java:76 dereferences `nets.get(netNumber)` unchecked.
        let rules = rules();
        rules.get_trace_half_width(1, 0);
    }

    #[test]
    fn new_net_class_inherits_from_the_default_class() {
        // BoardRules.java:146-162.
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
        // BoardRules.java:226-229.
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
        // BoardRules.java:212-219.
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
        // BoardRules.java:169-199. Padstacks 0 and 2 share the layer range 0..1, and padstack 2
        // is the smaller pad (100 - 10*2 = 80 < 100), so it wins. Padstack 1 has its own range.
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
        // Clearance class 2 does not match the default via clearance class (1), so it is skipped.
        rules
            .via_infos
            .add(ViaInfo::new("d", PadstackId(3), 2, false));

        rules.create_default_via_rule(default_class, "default", &TestPadstacks);

        assert_eq!(rules.via_rules.len(), 1);
        let rule = &rules.via_rules[0];
        assert_eq!(rule.name, "default");
        assert_eq!(rule.via_count(), 2);
        assert!(rule.contains(ViaInfoId(1)));
        assert!(rule.contains(ViaInfoId(2)));
        assert!(!rule.contains(ViaInfoId(0)));
        assert_eq!(
            rules.net_classes.get(default_class).get_via_rule(),
            Some(ViaRuleId(0))
        );
        assert_eq!(rules.get_default_via_rule(), Some(ViaRuleId(0)));
        assert_eq!(rules.get_via_rule("default"), Some(ViaRuleId(0)));
        assert_eq!(rules.get_via_rule("nope"), None);
    }

    #[test]
    fn create_default_via_rule_does_nothing_without_via_infos() {
        // BoardRules.java:170-172.
        let mut rules = rules();
        let default_class = rules.get_default_net_class();
        rules.create_default_via_rule(default_class, "default", &TestPadstacks);
        assert!(rules.via_rules.is_empty());
        assert_eq!(rules.net_classes.get(default_class).get_via_rule(), None);
    }

    #[test]
    fn default_via_diameter() {
        // BoardRules.java:408-421.
        let mut rules = rules();
        assert_eq!(rules.get_default_via_diameter(&TestPadstacks), 0.0);

        let default_class = rules.get_default_net_class();
        rules
            .via_infos
            .add(ViaInfo::new("a", PadstackId(3), 1, false));
        rules.create_default_via_rule(default_class, "default", &TestPadstacks);
        // Padstack 3's max width is 100 - 30 = 70 on both its layers.
        assert_eq!(rules.get_default_via_diameter(&TestPadstacks), 70.0);

        // A rule with no vias still answers 0 (BoardRules.java:413-415).
        rules.via_rules[0] = ViaRule::empty();
        assert_eq!(rules.get_default_via_diameter(&TestPadstacks), 0.0);
    }

    #[test]
    fn change_clearance_class_index_renumbers_items_net_classes_and_vias() {
        // BoardRules.java:263-289.
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
    fn remove_clearance_class_refuses_while_the_class_is_in_use() {
        // BoardRules.java:296-319.
        let mut rules = rules();
        rules.clearance_matrix.append_class("power");
        let default_class = rules.get_default_net_class();
        assert_eq!(rules.clearance_matrix.get_class_count(), 3);

        // A board item still on class 2 blocks the removal.
        let mut items = [TestItem {
            clearance_class_index: 2,
        }];
        assert!(!rules.remove_clearance_class(2, items.iter_mut()));
        assert_eq!(rules.clearance_matrix.get_class_count(), 3);

        // So does a net class: the default class's trace clearance class is 1.
        let mut items: Vec<TestItem> = Vec::new();
        assert!(!rules.remove_clearance_class(1, items.iter_mut()));

        // And so does a via info.
        rules
            .via_infos
            .add(ViaInfo::new("a", PadstackId(0), 2, false));
        assert!(!rules.remove_clearance_class(2, items.iter_mut()));

        // Freeing every reference lets it through, and higher indices shift down.
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
