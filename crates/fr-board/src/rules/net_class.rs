//! Net classes: the per-net routing rules, and the default per-item-type clearance classes.
//!
//! Java: `rules/NetClass.java`, `rules/NetClasses.java`,
//! `rules/DefaultItemClearanceClasses.java`.

use std::fmt;

use crate::ids::NetClassId;
use crate::rules::ViaRule;
use crate::structure::LayerStructure;

/// Port of `DefaultItemClearanceClasses.ItemClass`
/// (DefaultItemClearanceClasses.java:42-49): the item types for which a default clearance class
/// is stored.
///
/// Declaration order is Java's `ordinal()` order, which
/// [`DefaultItemClearanceClasses`] uses as an array index.
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
    /// Java's `ItemClass.values()`, which `BoardRules.changeClearanceClassIndex` and
    /// `BoardRules.removeClearanceClass` iterate (BoardRules.java:275-276,306-307,332-333).
    pub const VALUES: [ItemClass; 6] = [
        ItemClass::None,
        ItemClass::Trace,
        ItemClass::Via,
        ItemClass::Pin,
        ItemClass::Smd,
        ItemClass::Area,
    ];

    /// Java's `Enum.ordinal()` (DefaultItemClearanceClasses.java:23,31 index by it).
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

/// Port of `DefaultItemClearanceClasses` (`rules/DefaultItemClearanceClasses.java`): the default
/// clearance class index for each item type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DefaultItemClearanceClasses {
    /// `clearanceClasses` (DefaultItemClearanceClasses.java:8), one entry per [`ItemClass`].
    clearance_classes: [usize; ItemClass::VALUES.len()],
}

impl Default for DefaultItemClearanceClasses {
    fn default() -> Self {
        DefaultItemClearanceClasses::new()
    }
}

impl DefaultItemClearanceClasses {
    /// Port of the `DefaultItemClearanceClasses()` constructor
    /// (DefaultItemClearanceClasses.java:11-14): a zeroed array, then `setAll(1)`.
    ///
    /// Because `setAll` starts at index 1 (see [`Self::set_all`]), the result is
    /// `[0, 1, 1, 1, 1, 1]` — `ItemClass::None` keeps the "no clearance" class 0.
    pub fn new() -> DefaultItemClearanceClasses {
        let mut result = DefaultItemClearanceClasses {
            clearance_classes: [0; ItemClass::VALUES.len()],
        };
        result.set_all(1);
        result
    }

    /// Port of `DefaultItemClearanceClasses.get` (DefaultItemClearanceClasses.java:22-24).
    pub fn get(&self, item_class: ItemClass) -> usize {
        self.clearance_classes[item_class.ordinal()]
    }

    /// Port of `DefaultItemClearanceClasses.set` (DefaultItemClearanceClasses.java:30-32).
    ///
    /// Unlike [`Self::set_all`] this *can* write index 0 — `BoardRules.removeClearanceClass`
    /// relies on it when it renumbers classes (BoardRules.java:336).
    pub fn set(&mut self, item_class: ItemClass, index: usize) {
        self.clearance_classes[item_class.ordinal()] = index;
    }

    /// Port of `DefaultItemClearanceClasses.setAll` (DefaultItemClearanceClasses.java:35-39).
    ///
    /// Java's loop starts at `i = 1`, deliberately leaving `ItemClass.NONE` alone so that the
    /// "no clearance" item class keeps clearance class 0.
    pub fn set_all(&mut self, index: usize) {
        for i in 1..self.clearance_classes.len() {
            self.clearance_classes[i] = index;
        }
    }
}

/// Port of `NetClass` (`rules/NetClass.java`): the routing rules that apply to the nets in one
/// class.
///
/// not ported: `NetClass.clearanceMatrix` (NetClass.java:14) — the field is read only by
/// `NetClass.printInfo` (NetClass.java:220-224), so a net class needs no clearance matrix here.
///
/// not ported: `NetClass.boardLayerStructure` (NetClass.java:15) as a field. The two methods
/// that use it outside `printInfo`, [`Self::trace_width_is_layer_dependent`] and
/// [`Self::trace_width_is_inner_layer_dependent`], take the layer structure as an argument
/// instead — no rules object holds a board reference (Plan 2 design rule).
///
/// not ported: `NetClass.printInfo` (NetClass.java:213-244) — `ItemInfoPrinter.Printable`, GUI
/// only.
#[derive(Debug, Clone, PartialEq)]
pub struct NetClass {
    /// `NetClass.name` (NetClass.java:27).
    name: String,
    /// `NetClass.traceHalfWidthArr` (NetClass.java:16), one entry per board layer.
    trace_half_width: Vec<i32>,
    /// `NetClass.activeRoutingLayerArr` (NetClass.java:17), one entry per board layer.
    active_routing_layer: Vec<bool>,
    /// `NetClass.defaultItemClearanceClasses` (NetClass.java:23-24); public in Java too.
    pub default_item_clearance_classes: DefaultItemClearanceClasses,
    /// `NetClass.isIgnoredByAutorouter` (NetClass.java:26); public in Java too.
    pub is_ignored_by_autorouter: bool,
    /// `NetClass.viaRule` (NetClass.java:28), **owned**. Java's field starts out `null`, hence
    /// the `Option`.
    ///
    /// # Owned, not an index — ruling H's via-rule half (Plan 7 Task 11)
    ///
    /// Java's field is an object reference, and `Network.addViaRule` (Network.java:413-417)
    /// *removes* a replaced rule from `board.rules.viaRules` without telling any net class. A
    /// class that pointed at it therefore keeps the **detached original**, which is still what
    /// `AutorouteControl.initNet:210` reads and `rebuildViaInfo:234-284` turns into the router's
    /// via masks. A `ViaRuleId` into
    /// [`BoardRules::via_rules`](crate::rules::BoardRules::via_rules) cannot express that, and
    /// the divergence was **measured**, not argued — see
    /// [`BoardRules::replace_via_rule`](crate::rules::BoardRules::replace_via_rule).
    via_rule: Option<ViaRule>,
    /// `NetClass.traceClearanceClass` (NetClass.java:29).
    trace_clearance_class: usize,
    /// `NetClass.shoveFixed` (NetClass.java:32).
    shove_fixed: bool,
    /// `NetClass.pullTight` (NetClass.java:34), which Java initialises to `true`.
    pull_tight: bool,
    /// `NetClass.ignoreCyclesWithAreas` (NetClass.java:35).
    ignore_cycles_with_areas: bool,
    /// `NetClass.minimumTraceLength` (NetClass.java:36); `<= 0` means "no restriction".
    minimum_trace_length: f64,
    /// `NetClass.maximumTraceLength` (NetClass.java:37); `<= 0` means "no restriction".
    maximum_trace_length: f64,
}

impl NetClass {
    /// Port of the `NetClass(String, LayerStructure, ClearanceMatrix, boolean)` constructor
    /// (NetClass.java:40-54), minus the clearance matrix (see the type docs).
    ///
    /// Every layer starts active for routing exactly when it is a signal layer
    /// (NetClass.java:50-52).
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

    /// Port of `NetClass.getName` (NetClass.java:62-64).
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Port of `NetClass.setName` (NetClass.java:67-69).
    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }

    /// Port of `NetClass.setTraceHalfWidth(int)` (NetClass.java:72-74): all layers.
    pub fn set_trace_half_width_on_all_layers(&mut self, value: i32) {
        self.trace_half_width.fill(value);
    }

    /// Port of `NetClass.setTraceHalfWidth(int, int)` (NetClass.java:77-79): one layer. Java
    /// indexes unchecked and throws for an out-of-range layer; the port panics likewise.
    pub fn set_trace_half_width(&mut self, layer: usize, value: i32) {
        self.trace_half_width[layer] = value;
    }

    /// Port of `NetClass.setTraceHalfWidthOnInner` (NetClass.java:82-86): every layer except the
    /// first and the last.
    pub fn set_trace_half_width_on_inner(&mut self, value: i32) {
        for i in 1..self.trace_half_width.len().saturating_sub(1) {
            self.trace_half_width[i] = value;
        }
    }

    /// Port of `NetClass.layerCount` (NetClass.java:89-91).
    pub fn layer_count(&self) -> usize {
        self.trace_half_width.len()
    }

    /// Port of `NetClass.getTraceHalfWidth` (NetClass.java:94-100). Java warns and returns 0 for
    /// an out-of-range layer.
    pub fn get_trace_half_width(&self, layer: usize) -> i32 {
        self.trace_half_width.get(layer).copied().unwrap_or(0)
    }

    /// Port of `NetClass.getTraceClearanceClass` (NetClass.java:103-105).
    pub fn get_trace_clearance_class(&self) -> usize {
        self.trace_clearance_class
    }

    /// Port of `NetClass.setTraceClearanceClass` (NetClass.java:108-110).
    pub fn set_trace_clearance_class(&mut self, clearance_class: usize) {
        self.trace_clearance_class = clearance_class;
    }

    /// Port of `NetClass.getViaRule` (NetClass.java:113-115). `None` is Java's `null`.
    ///
    /// A borrow where Java hands back the reference itself; a caller that needs to keep the rule
    /// past the borrow clones it, which is what Java's reference copy does.
    pub fn get_via_rule(&self) -> Option<&ViaRule> {
        self.via_rule.as_ref()
    }

    /// Port of `NetClass.setViaRule` (NetClass.java:118-120).
    pub fn set_via_rule(&mut self, via_rule: Option<ViaRule>) {
        self.via_rule = via_rule;
    }

    /// Port of `NetClass.isShoveFixed` (NetClass.java:123-125).
    pub fn is_shove_fixed(&self) -> bool {
        self.shove_fixed
    }

    /// Port of `NetClass.setShoveFixed` (NetClass.java:128-130).
    pub fn set_shove_fixed(&mut self, value: bool) {
        self.shove_fixed = value;
    }

    /// Port of `NetClass.getPullTight` (NetClass.java:133-135).
    pub fn get_pull_tight(&self) -> bool {
        self.pull_tight
    }

    /// Port of `NetClass.setPullTight` (NetClass.java:138-140).
    pub fn set_pull_tight(&mut self, value: bool) {
        self.pull_tight = value;
    }

    /// Port of `NetClass.getIgnoreCyclesWithAreas` (NetClass.java:143-145).
    pub fn get_ignore_cycles_with_areas(&self) -> bool {
        self.ignore_cycles_with_areas
    }

    /// Port of `NetClass.setIgnoreCyclesWithAreas` (NetClass.java:148-150).
    pub fn set_ignore_cycles_with_areas(&mut self, value: bool) {
        self.ignore_cycles_with_areas = value;
    }

    /// Port of `NetClass.getMinimumTraceLength` (NetClass.java:156-158). A result `<= 0` means
    /// there is no minimum-length restriction.
    pub fn get_minimum_trace_length(&self) -> f64 {
        self.minimum_trace_length
    }

    /// Port of `NetClass.setMinimumTraceLength` (NetClass.java:164-166).
    pub fn set_minimum_trace_length(&mut self, value: f64) {
        self.minimum_trace_length = value;
    }

    /// Port of `NetClass.getMaximumTraceLength` (NetClass.java:172-174). A result `<= 0` means
    /// there is no maximum-length restriction.
    pub fn get_maximum_trace_length(&self) -> f64 {
        self.maximum_trace_length
    }

    /// Port of `NetClass.setMaximumTraceLength` (NetClass.java:180-182).
    pub fn set_maximum_trace_length(&mut self, value: f64) {
        self.maximum_trace_length = value;
    }

    /// Port of `NetClass.isActiveRoutingLayer` (NetClass.java:185-190): false for an
    /// out-of-range layer, as in Java.
    pub fn is_active_routing_layer(&self, layer_number: usize) -> bool {
        self.active_routing_layer
            .get(layer_number)
            .copied()
            .unwrap_or(false)
    }

    /// Port of `NetClass.setActiveRoutingLayer` (NetClass.java:193-198): a no-op for an
    /// out-of-range layer, as in Java.
    pub fn set_active_routing_layer(&mut self, layer_number: usize, active: bool) {
        if let Some(slot) = self.active_routing_layer.get_mut(layer_number) {
            *slot = active;
        }
    }

    /// Port of `NetClass.setAllLayersActive` (NetClass.java:201-203).
    pub fn set_all_layers_active(&mut self, value: bool) {
        self.active_routing_layer.fill(value);
    }

    /// Port of `NetClass.setAllInnerLayersActive` (NetClass.java:206-210).
    ///
    /// Java bounds the loop by `traceHalfWidthArr.length`, not by
    /// `activeRoutingLayerArr.length`; the two are always the same length (both are built from
    /// the layer structure in the constructor), so the port uses one length for both.
    pub fn set_all_inner_layers_active(&mut self, value: bool) {
        for i in 1..self.active_routing_layer.len().saturating_sub(1) {
            self.active_routing_layer[i] = value;
        }
    }

    /// Port of `NetClass.traceWidthIsLayerDependent` (NetClass.java:247-257): true if the trace
    /// width differs across the *signal* layers.
    ///
    /// Note Java's asymmetry: the comparison baseline is layer 0's half width whether or not
    /// layer 0 is a signal layer, while the scan from layer 1 up skips non-signal layers.
    pub fn trace_width_is_layer_dependent(&self, layer_structure: &LayerStructure) -> bool {
        let compare_value = self.trace_half_width[0];
        (1..self.trace_half_width.len()).any(|i| {
            layer_structure.layers[i].is_signal && self.trace_half_width[i] != compare_value
        })
    }

    /// Port of `NetClass.traceWidthIsInnerLayerDependent` (NetClass.java:260-281): true if the
    /// trace width differs across the inner signal layers.
    //
    // Java bug: the `while (!layers[firstInnerLayerNo].isSignal) ++firstInnerLayerNo;` scan at
    // NetClass.java:266-268 has no upper bound, so a stack of four or more layers with **no**
    // signal layer at index >= 1 (e.g. one signal layer on top and three non-signal layers
    // below) walks straight off the array and throws `ArrayIndexOutOfBoundsException`. The port
    // panics at the same slice index; see docs/java-quirks.md (#37).
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
    // renamed: NetClass.toString -> Display::fmt (NetClass.java:57-59 returns `name`).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

/// Port of `NetClasses` (`rules/NetClasses.java`): the board's array of net classes.
///
/// not ported: `NetClasses.remove` (NetClasses.java:105-107). Removing from the middle of the
/// `Vector` would renumber every later [`NetClassId`], and Java's only caller is
/// `gui/windows/routing/WindowNetClasses.java:385` — no `board`/`autoroute`/`drc`/`io` code
/// calls it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NetClasses {
    /// `NetClasses.classArr` (NetClasses.java:10).
    classes: Vec<NetClass>,
}

impl NetClasses {
    /// An empty net-class list; Java uses the implicit constructor plus the field initialiser at
    /// NetClasses.java:10.
    pub fn new() -> NetClasses {
        NetClasses::default()
    }

    /// Port of `NetClasses.count` (NetClasses.java:13-15).
    pub fn count(&self) -> usize {
        self.classes.len()
    }

    /// Port of `NetClasses.get(int)` (NetClasses.java:18-21). Java asserts the index is in range
    /// and would then throw; the port panics on the same out-of-range index.
    pub fn get(&self, index: NetClassId) -> &NetClass {
        &self.classes[index.0]
    }

    /// Mutable counterpart of [`Self::get`]; Java's `get` already hands back a mutable object.
    pub fn get_mut(&mut self, index: NetClassId) -> &mut NetClass {
        &mut self.classes[index.0]
    }

    /// Port of `NetClasses.get(String)` (NetClasses.java:24-31): the class with exactly this
    /// name (Java uses `equals`, not `equalsIgnoreCase`), or `None`.
    pub fn get_by_name(&self, name: &str) -> Option<&NetClass> {
        self.get_no(name).map(|id| self.get(id))
    }

    /// The index form of [`Self::get_by_name`]. Not a Java method: Java's callers keep the
    /// object reference, while this port needs the [`NetClassId`].
    pub fn get_no(&self, name: &str) -> Option<NetClassId> {
        self.classes
            .iter()
            .position(|c| c.name == name)
            .map(NetClassId)
    }

    /// Every net class, in index order. Not a Java method — Java iterates `classArr` directly
    /// (NetClasses.java:25,61,85).
    pub fn iter(&self) -> std::slice::Iter<'_, NetClass> {
        self.classes.iter()
    }

    /// Port of `NetClasses.append(String, LayerStructure, ClearanceMatrix, boolean)`
    /// (NetClasses.java:34-42), minus the clearance matrix (see [`NetClass`]).
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

    /// Port of `NetClasses.append(LayerStructure, ClearanceMatrix)` (NetClasses.java:45-53): the
    /// overload that generates `class1`, `class2`, ... until the name is free, and passes
    /// `ignoredByAutorouter = false`.
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

    /// Port of `NetClasses.find(int, int, ViaRule)` (NetClasses.java:60-77): the first class
    /// whose trace half width is `trace_half_width` on *every* layer and whose clearance class
    /// and via rule match.
    ///
    /// # `getViaRule() == viaRule` is a reference test in Java, and a value test here
    ///
    /// `NetClasses.java:63` (and `:87`) compares the two rules with `==`, i.e. by object
    /// identity. Since Plan 7 Task 11 a [`NetClass`] owns its rule, so this port compares by
    /// value — a deviation, and a caller-scoped one. The **only** caller in either language is
    /// `Network.read_net_scope`'s `(rule (width …))` arm (Network.java:1442-1445), which passes
    /// `getDefaultNetClass().getViaRule()`; and at that point in the read every net class holds
    /// the rule `Network.insertViaRules` gave them all (Network.java:392-394,
    /// `fr_dsn::parser::network::insert_via_rules`), which is one object.
    /// So the two comparisons agree on every DSN, and they can only disagree on a board where a
    /// non-default class holds a *distinct but value-equal* rule — which no reader builds.
    /// Recorded rather than hidden: `docs/java-quirks.md`'s ruling-H row carries it.
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

    /// Port of `NetClasses.find(int[], int, ViaRule)` (NetClasses.java:84-102): as
    /// [`Self::find`], but matching a per-layer trace half width array. A class whose layer count
    /// differs from the array length never matches (NetClasses.java:88).
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
        // DefaultItemClearanceClasses.java:13 calls setAll(1), whose loop starts at i = 1.
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
        // DefaultItemClearanceClasses.java:43-48.
        assert_eq!(ItemClass::None.ordinal(), 0);
        assert_eq!(ItemClass::Trace.ordinal(), 1);
        assert_eq!(ItemClass::Via.ordinal(), 2);
        assert_eq!(ItemClass::Pin.ordinal(), 3);
        assert_eq!(ItemClass::Smd.ordinal(), 4);
        assert_eq!(ItemClass::Area.ordinal(), 5);
    }

    #[test]
    fn new_net_class_activates_exactly_the_signal_layers() {
        // NetClass.java:50-52.
        let net_class = NetClass::new("default", &layers(), false);
        assert!(net_class.is_active_routing_layer(0));
        assert!(!net_class.is_active_routing_layer(1));
        assert!(net_class.is_active_routing_layer(2));
        assert!(net_class.is_active_routing_layer(3));
        // Out of range is false (NetClass.java:186-188).
        assert!(!net_class.is_active_routing_layer(4));
        // NetClass.java:34: pullTight defaults to true, everything else to Java's zero value.
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

        // NetClass.java:83: `for (i = 1; i < length - 1; i++)`.
        net_class.set_trace_half_width_on_inner(50);
        assert_eq!(
            (0..4)
                .map(|i| net_class.get_trace_half_width(i))
                .collect::<Vec<_>>(),
            vec![100, 50, 50, 100]
        );

        net_class.set_trace_half_width(3, 7);
        assert_eq!(net_class.get_trace_half_width(3), 7);
        // Out of range reads 0 (NetClass.java:95-98).
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
        // Out of range is a no-op (NetClass.java:194-196).
        net_class.set_active_routing_layer(99, true);
        assert!(!net_class.is_active_routing_layer(99));
    }

    #[test]
    fn trace_width_is_layer_dependent_skips_non_signal_layers() {
        // NetClass.java:250: only signal layers are compared, but the baseline is layer 0.
        let layers = layers();
        let mut net_class = NetClass::new("default", &layers, false);
        net_class.set_trace_half_width_on_all_layers(100);
        assert!(!net_class.trace_width_is_layer_dependent(&layers));

        // Changing the non-signal layer 1 is invisible to the check.
        net_class.set_trace_half_width(1, 999);
        assert!(!net_class.trace_width_is_layer_dependent(&layers));

        // Changing a signal layer is not.
        net_class.set_trace_half_width(2, 200);
        assert!(net_class.trace_width_is_layer_dependent(&layers));
    }

    #[test]
    fn trace_width_is_inner_layer_dependent() {
        // NetClass.java:262: `<= 3` layers is always false.
        let three = LayerStructure::new(vec![
            Layer::new("Top", true),
            Layer::new("Mid", true),
            Layer::new("Bottom", true),
        ]);
        let net_class = NetClass::new("default", &three, false);
        assert!(!net_class.trace_width_is_inner_layer_dependent(&three));

        // Four layers, first inner *signal* layer is index 2 (index 1 is not signal).
        let layers = layers();
        let mut net_class = NetClass::new("default", &layers, false);
        net_class.set_trace_half_width_on_all_layers(100);
        assert!(!net_class.trace_width_is_inner_layer_dependent(&layers));
        // firstInnerLayerNo == 2 == length - 2, so the scan range 3..3 is empty and layer 2's
        // own width is only the baseline: changing it alone cannot make this true.
        net_class.set_trace_half_width(2, 200);
        assert!(!net_class.trace_width_is_inner_layer_dependent(&layers));

        // A five-layer stack with two inner signal layers can differ.
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
        // Java bug (NetClass.java:266-268): the search for the first inner signal layer is
        // unbounded, so a >= 4 layer stack with no signal layer below the top runs off the end.
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
        // NetClasses.java:27 uses `equals`, so the lookup is case-sensitive.
        assert_eq!(net_classes.get_no("default"), Some(NetClassId(0)));
        assert_eq!(net_classes.get_no("DEFAULT"), None);
        assert!(net_classes.get_by_name("nope").is_none());
    }

    #[test]
    fn append_with_generated_name_skips_taken_names() {
        // NetClasses.java:47-51.
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

        // NetClasses.java:88: the per-layer form also requires equal layer counts.
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
