//! |---|---|
use std::collections::BTreeSet;

use copper_geometry::{Point, TileShape};

use crate::datastructures::StopCheck;
use crate::error::BoardError;
use crate::ids::{ItemId, TreeObject};
use crate::items::Item;
use crate::structure::FixedState;

use super::Board;

const PROTECT_FANOUT_LENGTH: f64 = 400.0;

impl Board {
    pub fn get_connectable_items(&self, net_number: i32) -> Vec<ItemId> {
        self.items
            .iter()
            .rev()
            .filter(|(_, item)| item.as_connectable().is_some() && item.contains_net(net_number))
            .map(|(id, _)| *id)
            .collect()
    }

    pub fn connectable_item_count(&self, net_number: i32) -> usize {
        self.items
            .values()
            .filter(|item| item.as_connectable().is_some() && item.contains_net(net_number))
            .count()
    }

    pub fn get_component_items(&self, component_id: i32) -> Vec<ItemId> {
        self.items
            .iter()
            .rev()
            .filter(|(_, item)| item.component_id() == component_id)
            .map(|(id, _)| *id)
            .collect()
    }

    pub fn get_component_pins(&self, component_id: i32) -> Vec<ItemId> {
        self.items
            .iter()
            .rev()
            .filter(|(_, item)| item.component_id() == component_id && matches!(item, Item::Pin(_)))
            .map(|(id, _)| *id)
            .collect()
    }

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

    pub fn get_connected_sets(&self, net_number: i32) -> Vec<BTreeSet<ItemId>> {
        let mut result = Vec::new();
        if net_number <= 0 {
            return result;
        }
        let mut items_to_handle: BTreeSet<ItemId> = self
            .items
            .iter()
            .filter(|(_, item)| item.as_connectable().is_some() && item.contains_net(net_number))
            .map(|(id, _)| *id)
            .collect();
        while let Some(current) = items_to_handle.iter().next_back().copied() {
            let next_connected_set = self.connected_set(current, net_number, false);
            for id in &next_connected_set {
                items_to_handle.remove(id);
            }
            items_to_handle.remove(&current);
            result.push(next_connected_set);
        }
        result
    }

    pub fn all_contacts(&self, id: ItemId) -> BTreeSet<ItemId> {
        self.all_contacts_impl(id, None)
    }

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
            if let Some(layer) = layer
                && shape_layer != layer
            {
                continue;
            }
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

    pub fn is_connected(&self, id: ItemId) -> bool {
        !self.all_contacts(id).is_empty()
    }

    pub fn is_connected_on_layer(&self, id: ItemId, layer: usize) -> bool {
        !self.all_contacts_on_layer(id, layer).is_empty()
    }

    pub fn normal_contacts(&self, id: ItemId) -> BTreeSet<ItemId> {
        let Some(item) = self.items.get(&id) else {
            return BTreeSet::new();
        };
        match item {
            Item::Trace(trace) => {
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
            if !other.shares_layer(item, &ctx) || !(ignore_net || other.shares_net(item)) {
                continue;
            }
            let is_contact = match other {
                Item::Trace(other_trace) => {
                    other_trace.first_corner().as_ref() == Some(point)
                        || other_trace.last_corner().as_ref() == Some(point)
                }
                Item::Via(_) | Item::Pin(_) => self.drill_center(other_id).as_ref() == Some(point),
                Item::ConductionArea(area) => area.get_area(&ctx).contains(point),
                _ => false,
            };
            if is_contact {
                result.insert(other_id);
            }
        }
        result
    }

    pub fn trace_start_contacts(&self, id: ItemId) -> BTreeSet<ItemId> {
        match self.items.get(&id) {
            Some(Item::Trace(trace)) => match trace.first_corner() {
                Some(corner) => self.trace_normal_contacts_at(id, &corner, false),
                None => BTreeSet::new(),
            },
            _ => BTreeSet::new(),
        }
    }

    pub fn trace_end_contacts(&self, id: ItemId) -> BTreeSet<ItemId> {
        match self.items.get(&id) {
            Some(Item::Trace(trace)) => match trace.last_corner() {
                Some(corner) => self.trace_normal_contacts_at(id, &corner, false),
                None => BTreeSet::new(),
            },
            _ => BTreeSet::new(),
        }
    }

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
            if !other.shares_net(item) || !other.shares_layer(item, &ctx) {
                continue;
            }
            let is_contact = match other {
                Item::Trace(other_trace) => {
                    other_trace.first_corner().as_ref() == Some(&drill_center)
                        || other_trace.last_corner().as_ref() == Some(&drill_center)
                }
                Item::Via(_) | Item::Pin(_) => {
                    self.drill_center(other_id).as_ref() == Some(&drill_center)
                }
                Item::ConductionArea(area) => area.get_area(&ctx).contains(&drill_center),
                _ => false,
            };
            if is_contact {
                result.insert(other_id);
            }
        }
        result
    }

    fn conduction_area_normal_contacts(&self, id: ItemId) -> BTreeSet<ItemId> {
        let mut result = BTreeSet::new();
        let Some(item @ Item::ConductionArea(area)) = self.items.get(&id) else {
            return result;
        };
        let ctx = self.ctx();
        let layer = area.get_layer();
        for i in 0..item.tile_shape_count(&ctx) {
            let Some(current_shape) = self.item_tile_shape_ref(id, i) else {
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
                if !other.shares_net(item) || !other.shares_layer(item, &ctx) {
                    continue;
                }
                let is_contact = match other {
                    Item::Trace(other_trace) => {
                        other_trace
                            .first_corner()
                            .is_some_and(|c| current_shape.contains(&c))
                            || other_trace
                                .last_corner()
                                .is_some_and(|c| current_shape.contains(&c))
                    }
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

    pub fn normal_contact_point(&self, a: ItemId, b: ItemId) -> Option<Point> {
        let item_a = self.items.get(&a)?;
        let item_b = self.items.get(&b)?;
        let ctx = self.ctx();
        match (item_a, item_b) {
            (Item::Trace(_), Item::Trace(trace_b)) => {
                let trace_a = match item_a {
                    Item::Trace(t) => t,
                    _ => unreachable!(),
                };
                if trace_b.get_layer() != trace_a.get_layer() {
                    return None;
                }
                let (b_first, b_last) = (trace_b.first_corner()?, trace_b.last_corner()?);
                let (a_first, a_last) = (trace_a.first_corner()?, trace_a.last_corner()?);
                let contact_at_first = b_first == a_first || b_first == a_last;
                let contact_at_last = b_last == a_first || b_last == a_last;
                if !(contact_at_first || contact_at_last) || (contact_at_first && contact_at_last) {
                    None
                } else if contact_at_first {
                    Some(b_first)
                } else {
                    Some(b_last)
                }
            }
            (Item::Trace(trace), Item::Via(_) | Item::Pin(_)) => {
                self.drill_trace_contact_point(b, trace, item_b, item_a, &ctx)
            }
            (Item::Via(_) | Item::Pin(_), Item::Trace(trace)) => {
                self.drill_trace_contact_point(a, trace, item_a, item_b, &ctx)
            }
            (Item::Via(_) | Item::Pin(_), Item::Via(_) | Item::Pin(_)) => {
                let center_a = self.drill_center(a)?;
                let center_b = self.drill_center(b)?;
                (item_b.shares_layer(item_a, &ctx) && center_b == center_a).then_some(center_b)
            }
            _ => None,
        }
    }

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

    pub fn first_common_layer(&self, a: ItemId, b: ItemId) -> Option<usize> {
        let item_a = self.items.get(&a)?;
        let item_b = self.items.get(&b)?;
        item_a.first_common_layer(item_b, &self.ctx())
    }

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
        if net_number > 0 && !item.contains_net(net_number) {
            return result;
        }
        result.insert(id);
        self.connected_set_recu(id, &mut result, net_number, stop_at_plane);
        result
    }

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
            if stop_at_plane
                && matches!(contact, Item::ConductionArea(_))
                && contact.component_id() <= 0
            {
                continue;
            }
            if net_number > 0 && !contact.contains_net(net_number) {
                continue;
            }
            if result.insert(contact_id) {
                self.connected_set_recu(contact_id, result, net_number, stop_at_plane);
            }
        }
    }

    pub fn unconnected_set(&self, id: ItemId, net_number: i32) -> BTreeSet<ItemId> {
        let mut result = BTreeSet::new();
        let Some(item) = self.items.get(&id) else {
            return result;
        };
        if net_number > 0 && !item.contains_net(net_number) {
            return result;
        }
        if net_number > 0 {
            result.extend(self.get_connectable_items(net_number));
        } else {
            for current_net_number in item.net_nos() {
                result.extend(self.get_connectable_items(*current_net_number));
            }
        }
        for connected in self.connected_set(id, net_number, false) {
            result.remove(&connected);
        }
        result
    }

    pub fn connection_items(
        &self,
        id: ItemId,
        stop_option: StopConnectionOption,
    ) -> BTreeSet<ItemId> {
        self.connection_items_checked(id, stop_option, &|| false)
            .expect("a `|| false` stop check never trips")
    }

    pub fn connection_items_checked(
        &self,
        id: ItemId,
        stop_option: StopConnectionOption,
        stop: StopCheck<'_>,
    ) -> Result<BTreeSet<ItemId>, BoardError> {
        let mut result = BTreeSet::new();
        let Some(item) = self.items.get(&id) else {
            return Ok(result);
        };
        let contacts = self.normal_contacts(id);
        if item.is_routable() {
            result.insert(id);
        }
        for start_contact in contacts.into_iter().rev() {
            let Some(mut prev_contact_point) = self.normal_contact_point(id, start_contact) else {
                continue;
            };
            let mut prev_contact_layer: i32 = self
                .first_common_layer(id, start_contact)
                .map_or(-1, |layer| layer as i32);
            if item.is_trace() {
                let check_contacts = self.trace_normal_contacts_at(id, &prev_contact_point, false);
                if check_contacts.len() != 1 {
                    continue;
                }
            }
            let mut visited: Vec<(ItemId, Point, i32, usize)> = Vec::new();
            let mut current_id = start_contact;
            loop {
                if stop() {
                    return Err(BoardError::Stopped);
                }
                match visited.iter_mut().find(|(item, point, layer, _)| {
                    *item == current_id
                        && *point == prev_contact_point
                        && *layer == prev_contact_layer
                }) {
                    Some((_, _, _, len_when_seen)) => {
                        if *len_when_seen == result.len() {
                            break;
                        }
                        *len_when_seen = result.len();
                    }
                    None => visited.push((
                        current_id,
                        prev_contact_point.clone(),
                        prev_contact_layer,
                        result.len(),
                    )),
                }
                let Some(current) = self.items.get(&current_id) else {
                    break;
                };
                if !current.is_routable() {
                    break;
                }
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
                let mut next_contact: Option<(ItemId, Point, i32)> = None;
                let mut fork_found = false;
                for tmp_contact in self.normal_contacts(current_id) {
                    let Some(tmp_contact_layer) = self.first_common_layer(current_id, tmp_contact)
                    else {
                        continue;
                    };
                    let tmp_contact_layer = tmp_contact_layer as i32;
                    let Some(tmp_contact_point) =
                        self.normal_contact_point(current_id, tmp_contact)
                    else {
                        fork_found = true;
                        break;
                    };
                    if prev_contact_layer != tmp_contact_layer
                        || prev_contact_point != tmp_contact_point
                    {
                        if next_contact.is_some() {
                            fork_found = true;
                            break;
                        }
                        next_contact = Some((tmp_contact, tmp_contact_point, tmp_contact_layer));
                    }
                }
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
        Ok(result)
    }

    pub fn is_tail(&self, id: ItemId) -> bool {
        let Some(item) = self.items.get(&id) else {
            return false;
        };
        match item {
            Item::Trace(_) => {
                self.trace_start_contacts(id).is_empty() || self.trace_end_contacts(id).is_empty()
            }
            Item::Via(_) => {
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

    pub fn is_cycle_recu(
        &self,
        id: ItemId,
        visited_items: &mut BTreeSet<ItemId>,
        search_item: ItemId,
        come_from_item: ItemId,
        ignore_areas: bool,
    ) -> bool {
        if ignore_areas && matches!(self.items.get(&id), Some(Item::ConductionArea(_))) {
            return false;
        }
        for contact_id in self.cycle_contacts(id).iter().rev().copied() {
            if contact_id == come_from_item {
                continue;
            }
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

    pub fn is_trace_cycle(&self, id: ItemId) -> bool {
        let Some(item @ Item::Trace(_)) = self.items.get(&id) else {
            return false;
        };
        if self.is_overlap(id) {
            return true;
        }
        let start_contacts = self.trace_start_contacts(id);
        let mut visited_items: BTreeSet<ItemId> = start_contacts.clone();
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
        for contact_id in start_contacts.into_iter().rev() {
            if self.is_cycle_recu(contact_id, &mut visited_items, id, id, ignore_areas) {
                return true;
            }
        }
        false
    }

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
            if is_lonely_smd_pin(contact_id) {
                return true;
            }
            let Some(Item::Trace(current_trace)) = self.items.get(&contact_id) else {
                continue;
            };
            if ignore_items.is_some_and(|set| set.contains(&contact_id)) {
                continue;
            }
            if current_trace.get_length()
                >= PROTECT_FANOUT_LENGTH * f64::from(current_trace.get_half_width())
            {
                continue;
            }
            for tmp_contact in self.normal_contacts(contact_id) {
                if is_lonely_smd_pin(tmp_contact) {
                    return true;
                }
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

    pub fn ratsnest_corners(&self, id: ItemId) -> Vec<Point> {
        let ctx = self.ctx();
        let Some(item) = self.items.get(&id) else {
            return Vec::new();
        };
        match item {
            Item::Trace(trace) => {
                let mut result = Vec::new();
                if self.trace_start_contacts(id).is_empty() {
                    match trace.first_corner() {
                        Some(corner) => result.push(corner),
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
            Item::Via(_) | Item::Pin(_) => self.drill_center(id).into_iter().collect(),
            Item::ConductionArea(area) => area
                .get_area(&ctx)
                .corner_approx_arr()
                .into_iter()
                .map(|corner| Point::Int(corner.round()))
                .collect(),
            _ => Vec::new(),
        }
    }

    pub fn swappable_pins(&self, id: ItemId) -> BTreeSet<ItemId> {
        let mut result = BTreeSet::new();
        let Some(Item::Pin(pin)) = self.items.get(&id) else {
            return result;
        };
        let component_id = pin.hdr.get_component_id();
        if component_id < 1 || component_id as usize > self.components.count() {
            return result;
        }
        let component = self.components.get(component_id);
        let Some(logical_part) = component.get_logical_part() else {
            return result;
        };
        let logical_part = self.library.logical_parts.get(logical_part);
        let pin_index = pin.get_pin_index();
        let Some(this_part_pin) = logical_part.get_pin(pin_index) else {
            return result;
        };
        if this_part_pin.gate_pin_swap_code <= 0 {
            return result;
        }
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
            if let Some(other_id) = self.get_pin(component_id, current_part_pin.pin_index) {
                result.insert(other_id);
            }
        }
        result
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StopConnectionOption {
    #[default]
    None,
    FanoutVia,
    Via,
}
