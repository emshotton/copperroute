//! The connectivity half of [`Board`]: Java's `BoardConnectivityQueries`
//! (`board/facade/BoardConnectivityQueries.java`) plus every `Item` method whose body reads
//! `this.board` to walk the search tree or the item list.
//!
//! # Where each body comes from
//!
//! | Java | here |
//! |---|---|
//! | `Item.getAllContacts()` / `(int)` (Item.java:495-548) | [`Board::all_contacts`], [`Board::all_contacts_on_layer`] |
//! | `Item.getNormalContacts()` + `Trace`/`DrillItem`/`ConductionArea` overrides | [`Board::normal_contacts`] |
//! | `Trace.getNormalContacts(Point, boolean)` (Trace.java:173-203) | [`Board::trace_normal_contacts_at`] |
//! | `Item.normalContactPoint` and its five overloads | [`Board::normal_contact_point`] |
//! | `Item.getConnectedSet` + `getConnectedSetRecu` (Item.java:591-635) | [`Board::connected_set`] |
//! | `Item.getUnconnectedSet` (Item.java:671-690) | [`Board::unconnected_set`] |
//! | `Item.getConnectionItems` (Item.java:692-781) | [`Board::connection_items`] |
//! | `Item.isTail` + `Trace`/`Via` overrides | [`Board::is_tail`] |
//! | `Item.isOverlap` + `Trace`'s override | [`Board::is_overlap`] |
//! | `Item.isCycleRecu` (Item.java:642-665), `Trace.isCycle` (Trace.java:272-330) | [`Board::is_cycle_recu`], [`Board::is_trace_cycle`] |
//! | `Item.isFanoutVia` (Item.java:1202-1239) | [`Board::is_fanout_via`] |
//! | `Item.getRatsnestCorners` + three overrides | [`Board::ratsnest_corners`] |
//! | `Pin.getSwappablePins` (Pin.java:391-427) | [`Board::swappable_pins`] |
//!
//! # The tree-shape cache
//!
//! The three bodies here that need an item's tile shapes (`getAllContacts`,
//! `ConductionArea.getNormalContacts`) read the *cache*
//! ([`Item::get_tile_shape`](crate::Item::get_tile_shape)) rather than
//! [`Board::item_tile_shape`], which would fill it. They are `&self` and the fill is `&mut self`.
//! The board's own insert protocol fills the cache for every tree
//! ([`Board::insert_item`] -> `SearchTreeManager::insert`), so the only way to reach a cold cache
//! here is an item whose `clearDerivedData()` ran without a re-insert — which on this board means
//! `Board::change_clearance_class_index` with clearance compensation off. Documented rather than
//! worked around; the `&mut self` query paths (`check_move_item`, `check_change_net`,
//! `validate_item`, `remove_items_marking_changed_area`) do use the lazy fill.
//!
//! # Set ordering
//!
//! Every one of these returns a `TreeSet<Item>` in Java, which iterates by `Item.compareTo` —
//! **descending** id (quirk #44). The port returns a `BTreeSet<ItemId>`, which iterates
//! *ascending*. The membership is identical and no ported body's *result* depends on the order
//! (the two that look like they might, `Via.isTail` and `getConnectionItems`, are argued in their
//! doc comments); where a Java caller does observe the order — `BasicBoard.getConnectedSets`'
//! outer sequence — the port re-derives it with `.rev()`.

use std::collections::BTreeSet;

use fr_geometry::{Point, TileShape};

use crate::ids::{ItemId, TreeObject};
use crate::items::Item;
use crate::structure::FixedState;

use super::Board;

// The `Board` methods below carry Java's method names without the `get_` prefix the rest of the
// crate drops, so each Java name is recorded here — one per line, because `scripts/audit-port.sh`
// matches `renamed:` and the Java name on the *same* physical line.
//
// renamed: `Item.getNormalContacts` (and the `Trace`, `DrillItem` and `ConductionArea` overrides) -> `Board::normal_contacts`.
// renamed: `Item.getAllContacts` (both overloads) -> `Board::all_contacts` / `Board::all_contacts_on_layer`.
// renamed: `Item.getConnectedSet` (both overloads) -> `Board::connected_set`.
// renamed: `Item.getUnconnectedSet` -> `Board::unconnected_set`.
// renamed: `Item.getConnectionItems` (both overloads) -> `Board::connection_items`.
// renamed: `Item.getRatsnestCorners` (and the `Trace`, `DrillItem` and `ConductionArea` overrides) -> `Board::ratsnest_corners`.
// renamed: `Item.getAllNets` -> `Board::all_nets`.
// renamed: `Item.getAllNetNames` -> `Board::all_net_names`.
// renamed: `Item.componentName` -> `Board::item_component_name`.
// renamed: `Item.moveBy` (and `DrillItem`'s override) -> `Board::move_item_by`.

/// `Item.PROTECT_FANOUT_LENGTH` (Item.java:40).
const PROTECT_FANOUT_LENGTH: f64 = 400.0;

impl Board {
    // -- BoardConnectivityQueries ----------------------------------------------------------------

    /// Port of `BoardConnectivityQueries.getConnectableItems`
    /// (BoardConnectivityQueries.java:23-35), which `BasicBoard.getConnectableItems`
    /// (BasicBoard.java:608-610) delegates to.
    pub fn get_connectable_items(&self, net_number: i32) -> Vec<ItemId> {
        self.items
            .iter()
            .rev()
            .filter(|(_, item)| item.as_connectable().is_some() && item.contains_net(net_number))
            .map(|(id, _)| *id)
            .collect()
    }

    /// Port of `BoardConnectivityQueries.connectableItemCount`
    /// (BoardConnectivityQueries.java:38-50).
    pub fn connectable_item_count(&self, net_number: i32) -> usize {
        self.items
            .values()
            .filter(|item| item.as_connectable().is_some() && item.contains_net(net_number))
            .count()
    }

    /// Port of `BoardConnectivityQueries.getComponentItems`
    /// (BoardConnectivityQueries.java:53-65).
    pub fn get_component_items(&self, component_id: i32) -> Vec<ItemId> {
        self.items
            .iter()
            .rev()
            .filter(|(_, item)| item.component_id() == component_id)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Port of `BoardConnectivityQueries.getComponentPins`
    /// (BoardConnectivityQueries.java:68-80).
    pub fn get_component_pins(&self, component_id: i32) -> Vec<ItemId> {
        self.items
            .iter()
            .rev()
            .filter(|(_, item)| item.component_id() == component_id && matches!(item, Item::Pin(_)))
            .map(|(id, _)| *id)
            .collect()
    }

    /// Port of `BoardConnectivityQueries.getPin` (BoardConnectivityQueries.java:83-96): the pin
    /// of `component_id` with package pin index `pin_index`.
    ///
    /// Java walks the item list, so it answers the **first** match in item-list order, i.e. the
    /// one with the highest id.
    pub fn get_pin(&self, component_id: i32, pin_index: i32) -> Option<ItemId> {
        self.items
            .iter()
            .rev()
            .find(|(_, item)| match item {
                Item::Pin(pin) => {
                    item.component_id() == component_id && pin.get_pin_index() == pin_index
                }
                _ => false,
            })
            .map(|(id, _)| *id)
    }

    /// Port of `BoardConnectivityQueries.getConnectedSets`
    /// (BoardConnectivityQueries.java:99-124), which `BasicBoard.getConnectedSets`
    /// (BasicBoard.java:910-912) delegates to: the net's connectable items partitioned into
    /// connected components.
    ///
    /// Java seeds a `TreeSet<Item>` and repeatedly takes its first element, so the sets come back
    /// in **descending** id of their seed item; the `.rev()` below reproduces that.
    pub fn get_connected_sets(&self, net_number: i32) -> Vec<BTreeSet<ItemId>> {
        let mut result = Vec::new();
        // BoardConnectivityQueries.java:101-103.
        if net_number <= 0 {
            return result;
        }
        let mut items_to_handle: BTreeSet<ItemId> = self
            .items
            .iter()
            .filter(|(_, item)| item.as_connectable().is_some() && item.contains_net(net_number))
            .map(|(id, _)| *id)
            .collect();
        // BoardConnectivityQueries.java:115-122: `itemsToHandle.iterator().next()` on a
        // descending `TreeSet`, so the highest remaining id seeds the next set.
        while let Some(current) = items_to_handle.iter().next_back().copied() {
            let next_connected_set = self.connected_set(current, net_number, false);
            for id in &next_connected_set {
                items_to_handle.remove(id);
            }
            // Java removes the whole connected set, which need not contain `current` if the item
            // was not on the net — it always is here, but remove it defensively so the loop
            // cannot spin.
            items_to_handle.remove(&current);
            result.push(next_connected_set);
        }
        result
    }

    // -- contacts --------------------------------------------------------------------------------

    /// Port of `Item.getAllContacts()` (Item.java:495-521): every connectable item that shares a
    /// net with this one and overlaps one of its tile shapes.
    ///
    /// Empty for an item that is not `Connectable` (Item.java:497-499).
    pub fn all_contacts(&self, id: ItemId) -> BTreeSet<ItemId> {
        self.all_contacts_impl(id, None)
    }

    /// Port of `Item.getAllContacts(int)` (Item.java:523-548): the same, restricted to the tile
    /// shapes on `layer`.
    pub fn all_contacts_on_layer(&self, id: ItemId, layer: usize) -> BTreeSet<ItemId> {
        self.all_contacts_impl(id, Some(layer))
    }

    fn all_contacts_impl(&self, id: ItemId, layer: Option<usize>) -> BTreeSet<ItemId> {
        let mut result = BTreeSet::new();
        let Some(item) = self.items.get(&id) else {
            return result;
        };
        if item.as_connectable().is_none() {
            return result;
        }
        let ctx = self.ctx();
        for i in 0..item.tile_shape_count(&ctx) {
            let shape_layer = item.shape_layer(i, &ctx);
            // Item.java:531-533: the layer overload skips the shapes on other layers.
            if let Some(layer) = layer
                && shape_layer != layer
            {
                continue;
            }
            // Item.java:505-506,536-537: `getTileShape(i)`, which recomputes on a cold cache.
            let Some(shape) = self.item_tile_shape_ref(id, i) else {
                continue;
            };
            for object in self.overlapping_objects(&shape, Some(shape_layer)) {
                let TreeObject::Item(other_id) = object else {
                    continue;
                };
                if other_id == id {
                    continue;
                }
                let Some(other) = self.items.get(&other_id) else {
                    continue;
                };
                if other.as_connectable().is_some() && other.shares_net(item) {
                    result.insert(other_id);
                }
            }
        }
        result
    }

    /// Port of `Item.isConnected` (Item.java:550-557).
    pub fn is_connected(&self, id: ItemId) -> bool {
        !self.all_contacts(id).is_empty()
    }

    /// Port of `Item.isConnectedOnLayer` (Item.java:559-566).
    pub fn is_connected_on_layer(&self, id: ItemId, layer: usize) -> bool {
        !self.all_contacts_on_layer(id, layer).is_empty()
    }

    /// Port of `Item.getNormalContacts` (Item.java:568-571) and its three overrides:
    /// `Trace` (Trace.java:154-166), `DrillItem` (DrillItem.java:273-306) and `ConductionArea`
    /// (ConductionArea.java:331-355). Anything else answers the empty base set.
    ///
    /// A "normal" contact is a connection at a *connection point* — a trace end corner, a drill
    /// item's centre, or a point inside a conduction area — as opposed to
    /// [`Board::all_contacts`], which counts any shape overlap.
    pub fn normal_contacts(&self, id: ItemId) -> BTreeSet<ItemId> {
        let Some(item) = self.items.get(&id) else {
            return BTreeSet::new();
        };
        match item {
            Item::Trace(trace) => {
                // Trace.java:156-165.
                let mut result = BTreeSet::new();
                if let Some(start_corner) = trace.first_corner() {
                    result.extend(self.trace_normal_contacts_at(id, &start_corner, false));
                }
                if let Some(end_corner) = trace.last_corner() {
                    result.extend(self.trace_normal_contacts_at(id, &end_corner, false));
                }
                result
            }
            Item::Via(_) | Item::Pin(_) => self.drill_item_normal_contacts(id),
            Item::ConductionArea(_) => self.conduction_area_normal_contacts(id),
            _ => BTreeSet::new(),
        }
    }

    /// Port of `Trace.getNormalContacts(Point, boolean)` (Trace.java:173-203).
    ///
    /// `point` must be one of the trace's own end corners, otherwise Java returns the empty set
    /// (Trace.java:174-176). With `ignore_net`, contacts of foreign nets count too — the flag is
    /// only set by `Item.clearanceViolations`' tie-pin check (Item.java:387) and by
    /// `PolylineTrace.combine`.
    pub fn trace_normal_contacts_at(
        &self,
        id: ItemId,
        point: &Point,
        ignore_net: bool,
    ) -> BTreeSet<ItemId> {
        let mut result = BTreeSet::new();
        let Some(Item::Trace(trace)) = self.items.get(&id) else {
            return result;
        };
        // Trace.java:174-176.
        if trace.first_corner().as_ref() != Some(point)
            && trace.last_corner().as_ref() != Some(point)
        {
            return result;
        }
        let item = &self.items[&id];
        let ctx = self.ctx();
        let layer = trace.get_layer();
        let search_shape = TileShape::Box(TileShape::get_instance_from_point(point));
        for object in self.overlapping_objects(&search_shape, Some(layer)) {
            let TreeObject::Item(other_id) = object else {
                continue;
            };
            if other_id == id {
                continue;
            }
            let Some(other) = self.items.get(&other_id) else {
                continue;
            };
            // Trace.java:184-186.
            if !other.shares_layer(item, &ctx) || !(ignore_net || other.shares_net(item)) {
                continue;
            }
            let is_contact = match other {
                // Trace.java:187-190.
                Item::Trace(other_trace) => {
                    other_trace.first_corner().as_ref() == Some(point)
                        || other_trace.last_corner().as_ref() == Some(point)
                }
                // Trace.java:191-194.
                Item::Via(_) | Item::Pin(_) => self.drill_center(other_id).as_ref() == Some(point),
                // Trace.java:195-198.
                Item::ConductionArea(area) => area.get_area(&ctx).contains(point),
                _ => false,
            };
            if is_contact {
                result.insert(other_id);
            }
        }
        result
    }

    /// Port of `Trace.getStartContacts` (Trace.java:108-110).
    pub fn trace_start_contacts(&self, id: ItemId) -> BTreeSet<ItemId> {
        match self.items.get(&id) {
            Some(Item::Trace(trace)) => match trace.first_corner() {
                Some(corner) => self.trace_normal_contacts_at(id, &corner, false),
                // Java's `firstCorner()` returning null makes `getNormalContacts` return the
                // empty set at Trace.java:174-176.
                None => BTreeSet::new(),
            },
            _ => BTreeSet::new(),
        }
    }

    /// Port of `Trace.getEndContacts` (Trace.java:116-118).
    pub fn trace_end_contacts(&self, id: ItemId) -> BTreeSet<ItemId> {
        match self.items.get(&id) {
            Some(Item::Trace(trace)) => match trace.last_corner() {
                Some(corner) => self.trace_normal_contacts_at(id, &corner, false),
                None => BTreeSet::new(),
            },
            _ => BTreeSet::new(),
        }
    }

    /// Port of `DrillItem.getNormalContacts` (DrillItem.java:273-306): everything touching the
    /// drill item's centre on **any** layer (Java passes `-1`, DrillItem.java:277).
    fn drill_item_normal_contacts(&self, id: ItemId) -> BTreeSet<ItemId> {
        let mut result = BTreeSet::new();
        let Some(drill_center) = self.drill_center(id) else {
            return result;
        };
        let item = &self.items[&id];
        let ctx = self.ctx();
        let search_shape = TileShape::Box(TileShape::get_instance_from_point(&drill_center));
        for object in self.overlapping_objects(&search_shape, None) {
            let TreeObject::Item(other_id) = object else {
                continue;
            };
            if other_id == id {
                continue;
            }
            let Some(other) = self.items.get(&other_id) else {
                continue;
            };
            // DrillItem.java:282.
            if !other.shares_net(item) || !other.shares_layer(item, &ctx) {
                continue;
            }
            let is_contact = match other {
                // DrillItem.java:283-291: exact endpoint matching, deliberately, see the Java
                // comment about false cycle detection.
                Item::Trace(other_trace) => {
                    other_trace.first_corner().as_ref() == Some(&drill_center)
                        || other_trace.last_corner().as_ref() == Some(&drill_center)
                }
                // DrillItem.java:292-295.
                Item::Via(_) | Item::Pin(_) => {
                    self.drill_center(other_id).as_ref() == Some(&drill_center)
                }
                // DrillItem.java:296-299.
                Item::ConductionArea(area) => area.get_area(&ctx).contains(&drill_center),
                _ => false,
            };
            if is_contact {
                result.insert(other_id);
            }
        }
        result
    }

    /// Port of `ConductionArea.getNormalContacts` (ConductionArea.java:331-355): the connectable
    /// items of the same net whose connection point falls inside one of the area's tile shapes.
    fn conduction_area_normal_contacts(&self, id: ItemId) -> BTreeSet<ItemId> {
        let mut result = BTreeSet::new();
        let Some(item @ Item::ConductionArea(area)) = self.items.get(&id) else {
            return result;
        };
        let ctx = self.ctx();
        let default_tree = self.default_tree_id();
        let layer = area.get_layer();
        for i in 0..item.tile_shape_count(&ctx) {
            let Some(current_shape) = item.get_tile_shape(default_tree, i, &ctx) else {
                continue;
            };
            for object in self.overlapping_objects(&current_shape, Some(layer)) {
                let TreeObject::Item(other_id) = object else {
                    continue;
                };
                if other_id == id {
                    continue;
                }
                let Some(other) = self.items.get(&other_id) else {
                    continue;
                };
                // ConductionArea.java:341.
                if !other.shares_net(item) || !other.shares_layer(item, &ctx) {
                    continue;
                }
                let is_contact = match other {
                    // ConductionArea.java:342-347.
                    Item::Trace(other_trace) => {
                        other_trace
                            .first_corner()
                            .is_some_and(|c| current_shape.contains(&c))
                            || other_trace
                                .last_corner()
                                .is_some_and(|c| current_shape.contains(&c))
                    }
                    // ConductionArea.java:348-351. Note a conduction area is **not** a contact of
                    // another conduction area here — Java tests only `Trace` and `DrillItem`.
                    Item::Via(_) | Item::Pin(_) => self
                        .drill_center(other_id)
                        .is_some_and(|c| current_shape.contains(&c)),
                    _ => false,
                };
                if is_contact {
                    result.insert(other_id);
                }
            }
        }
        result
    }

    /// Port of `Item.normalContactPoint(Item)` (Item.java:573-589) and the five overloads that
    /// implement its double dispatch: `Trace.normalContactPoint(Item|Trace|DrillItem)`
    /// (Trace.java:120-152) and `DrillItem.normalContactPoint(Item|DrillItem|Trace)`
    /// (DrillItem.java:326-350).
    ///
    /// `None` is Java's `null`: no contact point, or more than one.
    pub fn normal_contact_point(&self, a: ItemId, b: ItemId) -> Option<Point> {
        let item_a = self.items.get(&a)?;
        let item_b = self.items.get(&b)?;
        let ctx = self.ctx();
        match (item_a, item_b) {
            // Trace.java:122 dispatches to `other.normalContactPoint(this)`, so the *receiver*
            // is `b`; the corner it answers with is `b`'s, which equals `a`'s when they touch.
            (Item::Trace(_), Item::Trace(trace_b)) => {
                let trace_a = match item_a {
                    Item::Trace(t) => t,
                    _ => unreachable!(),
                };
                // Trace.java:131-152, with `this` = `trace_b` and `other` = `trace_a`.
                if trace_b.get_layer() != trace_a.get_layer() {
                    return None;
                }
                let (b_first, b_last) = (trace_b.first_corner()?, trace_b.last_corner()?);
                let (a_first, a_last) = (trace_a.first_corner()?, trace_a.last_corner()?);
                let contact_at_first = b_first == a_first || b_first == a_last;
                let contact_at_last = b_last == a_first || b_last == a_last;
                if !(contact_at_first || contact_at_last) || (contact_at_first && contact_at_last) {
                    // Trace.java:142-145: no contact point, or more than one.
                    None
                } else if contact_at_first {
                    Some(b_first)
                } else {
                    Some(b_last)
                }
            }
            // Trace.java:126-128 hands a `DrillItem` argument straight back to it, and
            // DrillItem.java:328 does the same for a `Trace`; both land in
            // `DrillItem.normalContactPoint(Trace)` (DrillItem.java:339-349).
            (Item::Trace(trace), Item::Via(_) | Item::Pin(_)) => {
                self.drill_trace_contact_point(b, trace, item_b, item_a, &ctx)
            }
            (Item::Via(_) | Item::Pin(_), Item::Trace(trace)) => {
                self.drill_trace_contact_point(a, trace, item_a, item_b, &ctx)
            }
            // DrillItem.java:331-337, with `this` = `b` and `other` = `a`.
            (Item::Via(_) | Item::Pin(_), Item::Via(_) | Item::Pin(_)) => {
                let center_a = self.drill_center(a)?;
                let center_b = self.drill_center(b)?;
                (item_b.shares_layer(item_a, &ctx) && center_b == center_a).then_some(center_b)
            }
            // Item.java:576-578 and the two auxiliary overloads (:582-588) all return null.
            _ => None,
        }
    }

    /// `DrillItem.normalContactPoint(Trace)` (DrillItem.java:339-349).
    fn drill_trace_contact_point(
        &self,
        drill_id: ItemId,
        trace: &crate::items::PolylineTrace,
        drill_item: &Item,
        trace_item: &Item,
        ctx: &crate::items::ItemCtx<'_>,
    ) -> Option<Point> {
        if !drill_item.shares_layer(trace_item, ctx) {
            return None;
        }
        let drill_center = self.drill_center(drill_id)?;
        (trace.first_corner().as_ref() == Some(&drill_center)
            || trace.last_corner().as_ref() == Some(&drill_center))
        .then_some(drill_center)
    }

    /// Port of `Item.firstCommonLayer` (Item.java:320-331) between two board items. Java's `-1`
    /// is `None`.
    pub fn first_common_layer(&self, a: ItemId, b: ItemId) -> Option<usize> {
        let item_a = self.items.get(&a)?;
        let item_b = self.items.get(&b)?;
        item_a.first_common_layer(item_b, &self.ctx())
    }

    // -- connected / unconnected sets ---------------------------------------------------------

    /// Port of `Item.getConnectedSet(int, boolean)` (Item.java:600-614) together with the private
    /// `getConnectedSetRecu` (Item.java:617-635): everything reachable from `id` through normal
    /// contacts.
    ///
    /// `net_number <= 0` ignores the net filter (Item.java:601). With `stop_at_plane`, the walk
    /// does not continue through a conduction area that belongs to no component
    /// (Item.java:622-626).
    pub fn connected_set(
        &self,
        id: ItemId,
        net_number: i32,
        stop_at_plane: bool,
    ) -> BTreeSet<ItemId> {
        let mut result = BTreeSet::new();
        let Some(item) = self.items.get(&id) else {
            return result;
        };
        // Item.java:602-604.
        if net_number > 0 && !item.contains_net(net_number) {
            return result;
        }
        result.insert(id);
        self.connected_set_recu(id, &mut result, net_number, stop_at_plane);
        result
    }

    /// Port of the private `Item.getConnectedSetRecu` (Item.java:617-635).
    ///
    /// Java recurses; so does this, and the depth is bounded by the number of items in the
    /// connected set because `result.add` guards every descent (Item.java:632).
    fn connected_set_recu(
        &self,
        id: ItemId,
        result: &mut BTreeSet<ItemId>,
        net_number: i32,
        stop_at_plane: bool,
    ) {
        for contact_id in self.normal_contacts(id) {
            let Some(contact) = self.items.get(&contact_id) else {
                continue;
            };
            // Item.java:622-626.
            if stop_at_plane
                && matches!(contact, Item::ConductionArea(_))
                && contact.component_id() <= 0
            {
                continue;
            }
            // Item.java:627-629.
            if net_number > 0 && !contact.contains_net(net_number) {
                continue;
            }
            if result.insert(contact_id) {
                self.connected_set_recu(contact_id, result, net_number, stop_at_plane);
            }
        }
    }

    /// Port of `Item.getUnconnectedSet(int)` (Item.java:671-690): the connectable items of the
    /// net that this item's connected set does **not** reach.
    pub fn unconnected_set(&self, id: ItemId, net_number: i32) -> BTreeSet<ItemId> {
        let mut result = BTreeSet::new();
        let Some(item) = self.items.get(&id) else {
            return result;
        };
        // Item.java:673-675.
        if net_number > 0 && !item.contains_net(net_number) {
            return result;
        }
        if net_number > 0 {
            result.extend(self.get_connectable_items(net_number));
        } else {
            // Item.java:679-682: every net the item is on.
            for current_net_number in item.net_nos() {
                result.extend(self.get_connectable_items(*current_net_number));
            }
        }
        // Item.java:684.
        for connected in self.connected_set(id, net_number, false) {
            result.remove(&connected);
        }
        result
    }

    /// Port of `Item.getConnectionItems(StopConnectionOption)` (Item.java:698-781): every trace
    /// and via from this item up to the next fork or terminal item.
    ///
    /// The three-argument overload `getConnectionItems()` (Item.java:693-695) is
    /// `stop_option = None`.
    ///
    /// The outer loop runs over `getNormalContacts()`, a `TreeSet<Item>`, i.e. **descending id**
    /// (quirk #44) — and the order is load-bearing under
    /// [`StopConnectionOption::FanoutVia`], because `isFanoutVia(result)` (Item.java:735) reads
    /// the partially built result set, so which chain is walked first changes what the later
    /// ones are allowed to cross.
    pub fn connection_items(
        &self,
        id: ItemId,
        stop_option: StopConnectionOption,
    ) -> BTreeSet<ItemId> {
        let mut result = BTreeSet::new();
        let Some(item) = self.items.get(&id) else {
            return result;
        };
        let contacts = self.normal_contacts(id);
        // Item.java:701-703.
        if item.is_routable() {
            result.insert(id);
        }
        for start_contact in contacts.into_iter().rev() {
            // Item.java:705-709.
            let Some(mut prev_contact_point) = self.normal_contact_point(id, start_contact) else {
                continue;
            };
            // Item.java:710. Java stores `-1` when there is no common layer and compares that
            // below, where it never equals a real layer — so the port keeps the signed value
            // rather than skipping the contact. (Unreachable in practice: every non-null
            // `normalContactPoint` above already required a shared layer.)
            let mut prev_contact_layer: i32 = self
                .first_common_layer(id, start_contact)
                .map_or(-1, |layer| layer as i32);
            // Item.java:711-719: for a trace, only continue if the start contact is the *only*
            // contact at that point.
            if item.is_trace() {
                let check_contacts = self.trace_normal_contacts_at(id, &prev_contact_point, false);
                if check_contacts.len() != 1 {
                    continue;
                }
            }
            // Item.java:721-777: walk along the contacts until the next fork or non-route item.
            let mut current_id = start_contact;
            loop {
                let Some(current) = self.items.get(&current_id) else {
                    break;
                };
                // Item.java:723-726.
                if !current.is_routable() {
                    break;
                }
                // Item.java:727-736.
                if matches!(current, Item::Via(_)) {
                    if stop_option == StopConnectionOption::Via {
                        break;
                    }
                    if stop_option == StopConnectionOption::FanoutVia
                        && self.is_fanout_via(current_id, Some(&result))
                    {
                        break;
                    }
                }
                result.insert(current_id);
                // Item.java:738-770.
                let mut next_contact: Option<(ItemId, Point, i32)> = None;
                let mut fork_found = false;
                for tmp_contact in self.normal_contacts(current_id) {
                    // Item.java:744-746: `tmpContactLayer >= 0` is the guard, i.e. `Some`.
                    let Some(tmp_contact_layer) = self.first_common_layer(current_id, tmp_contact)
                    else {
                        continue;
                    };
                    let tmp_contact_layer = tmp_contact_layer as i32;
                    let Some(tmp_contact_point) =
                        self.normal_contact_point(current_id, tmp_contact)
                    else {
                        // Item.java:748-752: no unique contact point is a fork.
                        fork_found = true;
                        break;
                    };
                    if prev_contact_layer != tmp_contact_layer
                        || prev_contact_point != tmp_contact_point
                    {
                        if next_contact.is_some() {
                            // Item.java:754-758: a second new contact.
                            fork_found = true;
                            break;
                        }
                        next_contact = Some((tmp_contact, tmp_contact_point, tmp_contact_layer));
                    }
                }
                // Item.java:771-774.
                let Some((next_id, next_point, next_layer)) = next_contact else {
                    break;
                };
                if fork_found {
                    break;
                }
                current_id = next_id;
                prev_contact_point = next_point;
                prev_contact_layer = next_layer;
            }
        }
        result
    }

    // -- tails, overlaps and cycles ----------------------------------------------------------------

    /// Port of `Item.isTail` (Item.java:783-786) and its two overrides, `Trace.isTail`
    /// (Trace.java:212-219) and `Via.isTail` (Via.java:170-187).
    ///
    /// `Via.isTail`'s body iterates the contact list and compares every later contact's layer
    /// span against the **first** one's; the predicate it computes ("all contacts span the same
    /// layers") does not depend on which contact is first, so the port's ascending set order is
    /// safe.
    pub fn is_tail(&self, id: ItemId) -> bool {
        let Some(item) = self.items.get(&id) else {
            return false;
        };
        match item {
            Item::Trace(_) => {
                // Trace.java:213-218.
                self.trace_start_contacts(id).is_empty() || self.trace_end_contacts(id).is_empty()
            }
            Item::Via(_) => {
                // Via.java:171-186.
                let contacts = self.normal_contacts(id);
                if contacts.len() <= 1 {
                    return true;
                }
                let ctx = self.ctx();
                let mut spans = contacts.iter().filter_map(|c| {
                    self.items
                        .get(c)
                        .map(|item| (item.first_layer(&ctx), item.last_layer(&ctx)))
                });
                let Some(first) = spans.next() else {
                    return true;
                };
                spans.all(|span| span == first)
            }
            _ => false,
        }
    }

    /// Port of `Item.isOverlap` (Item.java:637-640) and its one override, `Trace.isOverlap`
    /// (Trace.java:227-232): a trace connected to the *same* item at both of its ends.
    pub fn is_overlap(&self, id: ItemId) -> bool {
        match self.items.get(&id) {
            Some(Item::Trace(_)) => {
                let start = self.trace_start_contacts(id);
                let end = self.trace_end_contacts(id);
                !start.is_disjoint(&end)
            }
            _ => false,
        }
    }

    /// Port of the package-private `Item.isCycleRecu` (Item.java:643-665): depth-first search
    /// from this item for `search_item`, not going back through `come_from_item`.
    pub fn is_cycle_recu(
        &self,
        id: ItemId,
        visited_items: &mut BTreeSet<ItemId>,
        search_item: ItemId,
        come_from_item: ItemId,
        ignore_areas: bool,
    ) -> bool {
        // Item.java:645-647.
        if ignore_areas && matches!(self.items.get(&id), Some(Item::ConductionArea(_))) {
            return false;
        }
        for contact_id in self.normal_contacts(id) {
            // Item.java:653-655.
            if contact_id == come_from_item {
                continue;
            }
            // Item.java:656-658.
            if contact_id == search_item {
                return true;
            }
            if visited_items.insert(contact_id)
                && self.is_cycle_recu(contact_id, visited_items, search_item, id, ignore_areas)
            {
                return true;
            }
        }
        false
    }

    /// Port of `Trace.isCycle` (Trace.java:272-330): can this trace be reached from itself by
    /// more than one path?
    ///
    /// Java's `FRLogger.trace` debug blocks for net 49 (:273-292,311-325) are dropped.
    pub fn is_trace_cycle(&self, id: ItemId) -> bool {
        let Some(item @ Item::Trace(_)) = self.items.get(&id) else {
            return false;
        };
        // Trace.java:275-294.
        if self.is_overlap(id) {
            return true;
        }
        let start_contacts = self.trace_start_contacts(id);
        // Trace.java:301: seeding `visitedItems` with every start contact is what blocks the
        // search from coming back into this trace through another start contact.
        let mut visited_items: BTreeSet<ItemId> = start_contacts.clone();
        // Trace.java:302-308.
        let mut ignore_areas = false;
        if let Some(first_net) = item.net_nos().first()
            && let Some(net) = self.rules.nets.get(*first_net)
        {
            ignore_areas = self
                .rules
                .net_classes
                .get(net.get_net_class())
                .get_ignore_cycles_with_areas();
        }
        for contact_id in start_contacts {
            if self.is_cycle_recu(contact_id, &mut visited_items, id, id, ignore_areas) {
                return true;
            }
        }
        false
    }

    /// Port of `Item.isFanoutVia(Set<Item>)` (Item.java:1206-1239): is this via connected,
    /// directly or through a short trace, to a nearby SMD pin?
    pub fn is_fanout_via(&self, id: ItemId, ignore_items: Option<&BTreeSet<ItemId>>) -> bool {
        let ctx = self.ctx();
        let is_lonely_smd_pin = |candidate: ItemId| -> bool {
            let Some(item) = self.items.get(&candidate) else {
                return false;
            };
            matches!(item, Item::Pin(_))
                && item.first_layer(&ctx) == item.last_layer(&ctx)
                && self.normal_contacts(candidate).len() <= 1
        };
        for contact_id in self.normal_contacts(id) {
            // Item.java:1208-1212.
            if is_lonely_smd_pin(contact_id) {
                return true;
            }
            let Some(Item::Trace(current_trace)) = self.items.get(&contact_id) else {
                continue;
            };
            // Item.java:1214-1216.
            if ignore_items.is_some_and(|set| set.contains(&contact_id)) {
                continue;
            }
            // Item.java:1217-1219.
            if current_trace.get_length()
                >= PROTECT_FANOUT_LENGTH * f64::from(current_trace.get_half_width())
            {
                continue;
            }
            for tmp_contact in self.normal_contacts(contact_id) {
                // Item.java:1222-1226.
                if is_lonely_smd_pin(tmp_contact) {
                    return true;
                }
                // Item.java:1227-1234: a shove-fixed two-corner exit trace of an SMD pin.
                if let Some(item @ Item::Trace(contact_trace)) = self.items.get(&tmp_contact)
                    && item.get_fixed_state() == FixedState::ShoveFixed
                    && contact_trace.corner_count() == 2
                {
                    return true;
                }
            }
        }
        false
    }

    /// Port of `Item.getRatsnestCorners` (Item.java:788-794) and its three overrides:
    /// `Trace` (Trace.java:337-368), `DrillItem` (DrillItem.java:351-356) and `ConductionArea`
    /// (ConductionArea.java:367-377).
    pub fn ratsnest_corners(&self, id: ItemId) -> Vec<Point> {
        let ctx = self.ctx();
        let Some(item) = self.items.get(&id) else {
            return Vec::new();
        };
        match item {
            Item::Trace(trace) => {
                // Trace.java:342-367: only the *uncontacted* end points.
                let mut result = Vec::new();
                if self.trace_start_contacts(id).is_empty() {
                    match trace.first_corner() {
                        Some(corner) => result.push(corner),
                        // Trace.java:362-366: a null corner makes the whole result empty.
                        None => return Vec::new(),
                    }
                }
                if self.trace_end_contacts(id).is_empty() {
                    match trace.last_corner() {
                        Some(corner) => result.push(corner),
                        None => return Vec::new(),
                    }
                }
                result
            }
            // DrillItem.java:352-356.
            Item::Via(_) | Item::Pin(_) => self.drill_center(id).into_iter().collect(),
            // ConductionArea.java:368-377.
            Item::ConductionArea(area) => area
                .get_area(&ctx)
                .corner_approx_arr()
                .into_iter()
                .map(|corner| Point::Int(corner.round()))
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Port of `Pin.getSwappablePins` (Pin.java:391-427): the pins of the same component whose
    /// part pin shares this pin's gate name and gate pin swap code.
    ///
    /// Java returns a `TreeSet<Pin>` — descending id (quirk #44); the port's `BTreeSet<ItemId>`
    /// is ascending, and no caller reads the first element.
    ///
    /// Java's `board.getPin(componentId, pinIndex)` (Pin.java:418) is [`Board::get_pin`], which
    /// answers `null` for a package pin that has no board pin; those are skipped
    /// (Pin.java:419-421).
    pub fn swappable_pins(&self, id: ItemId) -> BTreeSet<ItemId> {
        let mut result = BTreeSet::new();
        let Some(Item::Pin(pin)) = self.items.get(&id) else {
            return result;
        };
        let component_id = pin.hdr.get_component_id();
        // Pin.java:394-400: no component, or the component has no logical part, means no swap.
        if component_id < 1 || component_id as usize > self.components.count() {
            return result;
        }
        let component = self.components.get(component_id);
        let Some(logical_part) = component.get_logical_part() else {
            return result;
        };
        let logical_part = self.library.logical_parts.get(logical_part);
        let pin_index = pin.get_pin_index();
        // Pin.java:402-405. `LogicalPart.getPin(int)` indexes the array (LogicalPart.java:31-37),
        // and the doc contract is that a part pin's array index equals its package pin index.
        let Some(this_part_pin) = logical_part.get_pin(pin_index) else {
            return result;
        };
        // Pin.java:406-408.
        if this_part_pin.gate_pin_swap_code <= 0 {
            return result;
        }
        // Pin.java:410-425.
        for i in 0..logical_part.pin_count() {
            let i = i as i32;
            if i == pin_index {
                continue;
            }
            let Some(current_part_pin) = logical_part.get_pin(i) else {
                continue;
            };
            if current_part_pin.gate_pin_swap_code != this_part_pin.gate_pin_swap_code
                || current_part_pin.gate_name != this_part_pin.gate_name
            {
                continue;
            }
            // Pin.java:418-423: a package pin with no board pin is skipped (Java warns).
            if let Some(other_id) = self.get_pin(component_id, current_part_pin.pin_index) {
                result.insert(other_id);
            }
        }
        result
    }

    // added in Plan 5: `Item.clearanceViolations` (Item.java:363-469), `clearanceViolationCount`
    // (:357-361), the private `calculateClearanceBetweenTwoShapes` (:471-493) and `Via`'s
    // override (Via.java:88-112) -> `Board::clearance_violations` /
    // `Board::clearance_violation_count`. They build `drc.ClearanceViolation` objects, which is
    // the DRC layer Plan 5 owns; the search-tree query they need
    // (`overlapping_tree_entries_with_clearance`) is already here.
}

/// Port of `Item.StopConnectionOption` (Item.java:1295-1299): where
/// [`Board::connection_items`] stops following a connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StopConnectionOption {
    #[default]
    None,
    FanoutVia,
    Via,
}
