use std::collections::BTreeSet;

use fr_board::{Board, ConnectionId, Item, ItemId};
use fr_geometry::Point;

use crate::Arena;
use crate::autoroute::item_info;

const DETOUR_ADD: f64 = 100.0;

const DETOUR_ITEM_COST: f64 = 0.1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connection {
    pub start_point: Option<Point>,
    pub start_layer: i32,
    pub end_point: Option<Point>,
    pub end_layer: i32,
    pub item_list: BTreeSet<ItemId>,
}

impl Connection {
    pub fn get(
        board: &mut Board,
        connections: &mut Arena<Connection>,
        item: ItemId,
    ) -> Option<ConnectionId> {
        if !board.get_item(item)?.is_routable() {
            return None;
        }
        if let Some(precalculated) = item_info::get_precalculated_connection(board, item) {
            return Some(precalculated);
        }
        let contacts = board.normal_contacts(item);
        let mut connection_items: BTreeSet<ItemId> = BTreeSet::new();
        connection_items.insert(item);

        let mut start_point: Option<Point> = None;
        let mut start_layer = 0i32;
        let mut end_point: Option<Point> = None;
        let mut end_layer = 0i32;

        for start_contact in contacts.into_iter().rev() {
            let Some(mut prev_contact_point) = board.normal_contact_point(item, start_contact)
            else {
                continue;
            };
            let mut prev_contact_layer = layer_no(board.first_common_layer(item, start_contact));
            let mut fork_found = false;
            if matches!(board.get_item(item), Some(Item::Trace(_)))
                && board
                    .trace_normal_contacts_at(item, &prev_contact_point, false)
                    .len()
                    != 1
            {
                fork_found = true;
            }

            let mut current_item = start_contact;
            loop {
                let routable = board.get_item(current_item).is_some_and(Item::is_routable);
                if !routable || fork_found {
                    if start_point.is_none() {
                        start_point = Some(prev_contact_point.clone());
                        start_layer = prev_contact_layer;
                    } else if start_point.as_ref() != Some(&prev_contact_point) {
                        end_point = Some(prev_contact_point.clone());
                        end_layer = prev_contact_layer;
                    }
                    break;
                }
                connection_items.insert(current_item);
                let current_item_contacts = board.normal_contacts(current_item);
                let mut next_contact_point: Option<Point> = None;
                let mut next_contact_layer = -1i32;
                let mut next_contact: Option<ItemId> = None;
                for tmp_contact in current_item_contacts.into_iter().rev() {
                    let tmp_contact_layer =
                        layer_no(board.first_common_layer(current_item, tmp_contact));
                    if tmp_contact_layer >= 0 {
                        let Some(tmp_contact_point) =
                            board.normal_contact_point(current_item, tmp_contact)
                        else {
                            fork_found = true;
                            break;
                        };
                        if prev_contact_layer != tmp_contact_layer
                            || prev_contact_point != tmp_contact_point
                        {
                            next_contact_point = Some(tmp_contact_point);
                            next_contact_layer = tmp_contact_layer;
                            if next_contact.is_some() {
                                fork_found = true;
                                break;
                            }
                            next_contact = Some(tmp_contact);
                        }
                    }
                }
                let Some(next) = next_contact else {
                    break;
                };
                current_item = next;
                prev_contact_point = next_contact_point.expect("set beside `nextContact`");
                prev_contact_layer = next_contact_layer;
            }
        }

        let result = Connection {
            start_point,
            start_layer,
            end_point,
            end_layer,
            item_list: connection_items.clone(),
        };
        let id = ConnectionId(connections.insert(result));
        for current_item in connection_items {
            item_info::set_precalculated_connection(board, current_item, Some(id));
        }
        Some(id)
    }

    pub fn trace_length(&self, board: &Board) -> f64 {
        let mut result = 0.0;
        for current_item in &self.item_list {
            if let Some(Item::Trace(trace)) = board.get_item(*current_item) {
                result += trace.get_length();
            }
        }
        result
    }

    pub fn get_detour(&self, board: &Board) -> f64 {
        let (Some(start_point), Some(end_point)) = (&self.start_point, &self.end_point) else {
            return f64::from(i32::MAX);
        };
        let min_trace_length = start_point.to_float().distance(&end_point.to_float());
        (self.trace_length(board) + DETOUR_ADD) / (min_trace_length + DETOUR_ADD)
            + DETOUR_ITEM_COST * (self.item_list.len() as f64 - 1.0)
    }
}

fn layer_no(layer: Option<usize>) -> i32 {
    layer.map_or(-1, |l| i32::try_from(l).unwrap_or(i32::MAX))
}
