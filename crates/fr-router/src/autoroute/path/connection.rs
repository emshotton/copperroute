//! Port of `autoroute.path.Connection` (Connection.java:11-155) — "describes a routing connection
//! ending at the next fork or terminal item".

use std::collections::BTreeSet;

use fr_board::{Board, ConnectionId, Item, ItemId};
use fr_geometry::Point;

use crate::Arena;
use crate::autoroute::item_info;

/// `private static final double DETOUR_ADD = 100` (Connection.java:13).
const DETOUR_ADD: f64 = 100.0;

/// `private static final double DETOUR_ITEM_COST = 0.1` (Connection.java:14).
const DETOUR_ITEM_COST: f64 = 0.1;

/// Port of `path.Connection` (Connection.java:11-155).
///
/// # Where it lives
///
/// Java's is a heap object every member item points at through
/// `ItemAutorouteInfo.precalculatedConnection`. The port stores it in an
/// [`Arena<Connection>`](crate::Arena) on
/// [`AutorouteEngine`](crate::autoroute::maze::AutorouteEngine) and the item holds a
/// [`ConnectionId`] (plan-6 ruling 15, the same split `ObstacleExpansionRoom` already makes).
/// [`get`](Self::get) therefore takes the arena as well as the board.
///
/// # `itemList` is ascending here and descending in Java
///
/// Java's is a `TreeSet<Item>`, i.e. `Item.compareTo` = `other.id - this.id`, **descending** id
/// (Item.java:95-101); a [`BTreeSet<ItemId>`] is ascending. Nothing reads the order: the two
/// consumers are `itemList.size()` (`:139`, `:153`) and the `traceLength()` sum (`:133-141`),
/// both order-independent, and the memo loop of `:126-128` writes the same value into every
/// member. The `checkRipup` trace payload that *does* print the ids in order (`:147-155`) is one
/// of the `FRLogger` payloads plan-6's global constraints drop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connection {
    /// `public final Point startPoint` (`:17`): "if the connection ends in empty space,
    /// startPoint or endPoint may be null."
    pub start_point: Option<Point>,
    /// `public final int startLayer` (`:19`).
    ///
    /// `i32`, not `usize`: it is assigned from `item.firstCommonLayer(currentItem)` (`:62`), which
    /// answers `-1` for two items with no common layer (Item.java), and `:80` copies that through
    /// with no guard. The field's initialiser is `0` (`:52`).
    pub start_layer: i32,
    /// `public final Point endPoint` (`:20`).
    pub end_point: Option<Point>,
    /// `public final int endLayer` (`:21`).
    pub end_layer: i32,
    /// `public final Set<Item> itemList` (`:22`) — see the type docs for the order.
    pub item_list: BTreeSet<ItemId>,
}

impl Connection {
    /// Port of the static `get(Item)` (Connection.java:39-130): "gets the connection this item
    /// belongs to. A connection ends at the next fork or terminal item. Returns null if item is
    /// not a route item, or if it is a via belonging to more than 1 connection."
    ///
    /// `None` is Java's `null`, which here means only the `!item.isRoutable()` refusal of
    /// `:40-42` — every other path builds a `Connection`. (The doc comment's "or if it is a via
    /// belonging to more than 1 connection" describes an outcome of the walk, not a second
    /// `return null`.)
    ///
    /// Both `getAutorouteInfo()` calls (`:43`, `:127`) **create** the scratch on demand
    /// (Item.java:1038-1044), so a `get` on an item the router has not touched yet allocates one
    /// — which is why `crate::autoroute::item_info`'s accessors take `&mut Board`.
    ///
    /// # Iteration orders
    ///
    /// `getNormalContacts()` (`:47`, `:88`) answers a `TreeSet<Item>` — **descending** id — and
    /// `Trace.getNormalContacts(Point, boolean)` (`:68`) another. The port's
    /// `Board::normal_contacts` / `Board::trace_normal_contacts_at` are `BTreeSet`s, so every walk
    /// is `.rev()`. It is observable: `:96-116` picks the *first* new contact and calls the second
    /// one a fork, and `:56` decides which end becomes `startPoint`.
    pub fn get(
        board: &mut Board,
        connections: &mut Arena<Connection>,
        item: ItemId,
    ) -> Option<ConnectionId> {
        // :40-42.
        if !board.get_item(item)?.is_routable() {
            return None;
        }
        // :43-46.
        if let Some(precalculated) = item_info::get_precalculated_connection(board, item) {
            return Some(precalculated);
        }
        // :47-49.
        let contacts = board.normal_contacts(item);
        let mut connection_items: BTreeSet<ItemId> = BTreeSet::new();
        connection_items.insert(item);

        // :51-54.
        let mut start_point: Option<Point> = None;
        let mut start_layer = 0i32;
        let mut end_point: Option<Point> = None;
        let mut end_layer = 0i32;

        // :56.
        for start_contact in contacts.into_iter().rev() {
            // :57-61.
            let Some(mut prev_contact_point) = board.normal_contact_point(item, start_contact)
            else {
                // "no unique contact point"
                continue;
            };
            // :62. Java's `-1` for "no common layer".
            let mut prev_contact_layer = layer_no(board.first_common_layer(item, start_contact));
            let mut fork_found = false;
            // :64-72. "Check, that there is only 1 contact at this location. Only for pins and
            // vias items of more than 1 connection are collected."
            if matches!(board.get_item(item), Some(Item::Trace(_)))
                && board
                    .trace_normal_contacts_at(item, &prev_contact_point, false)
                    .len()
                    != 1
            {
                fork_found = true;
            }

            // :73-123. "Search from currentItem along the contacts until the next fork or
            // nonroute item."
            let mut current_item = start_contact;
            loop {
                // :76-86.
                let routable = board.get_item(current_item).is_some_and(Item::is_routable);
                if !routable || fork_found {
                    // "connection ends"
                    if start_point.is_none() {
                        start_point = Some(prev_contact_point.clone());
                        start_layer = prev_contact_layer;
                    } else if start_point.as_ref() != Some(&prev_contact_point) {
                        end_point = Some(prev_contact_point.clone());
                        end_layer = prev_contact_layer;
                    }
                    break;
                }
                // :87.
                connection_items.insert(current_item);
                // :88.
                let current_item_contacts = board.normal_contacts(current_item);
                // :93-95. "filter the contacts at the previous contact point, because we were
                // already there. If then there is not exactly 1 new contact left, there is a stub
                // or a fork."
                let mut next_contact_point: Option<Point> = None;
                let mut next_contact_layer = -1i32;
                let mut next_contact: Option<ItemId> = None;
                for tmp_contact in current_item_contacts.into_iter().rev() {
                    // :97-98.
                    let tmp_contact_layer =
                        layer_no(board.first_common_layer(current_item, tmp_contact));
                    if tmp_contact_layer >= 0 {
                        // :99-104.
                        let Some(tmp_contact_point) =
                            board.normal_contact_point(current_item, tmp_contact)
                        else {
                            // "no unique contact point"
                            fork_found = true;
                            break;
                        };
                        // :105-114.
                        if prev_contact_layer != tmp_contact_layer
                            || prev_contact_point != tmp_contact_point
                        {
                            next_contact_point = Some(tmp_contact_point);
                            next_contact_layer = tmp_contact_layer;
                            if next_contact.is_some() {
                                // "second new contact found"
                                fork_found = true;
                                break;
                            }
                            next_contact = Some(tmp_contact);
                        }
                    }
                }
                // :117-119.
                let Some(next) = next_contact else {
                    break;
                };
                // :120-122.
                current_item = next;
                prev_contact_point = next_contact_point.expect("set beside `nextContact`");
                prev_contact_layer = next_contact_layer;
            }
        }

        // :125.
        let result = Connection {
            start_point,
            start_layer,
            end_point,
            end_layer,
            item_list: connection_items.clone(),
        };
        let id = ConnectionId(connections.insert(result));
        // :126-128.
        for current_item in connection_items {
            item_info::set_precalculated_connection(board, current_item, Some(id));
        }
        // :129.
        Some(id)
    }

    /// Port of `traceLength()` (Connection.java:132-141): "returns the cumulative length of the
    /// traces in this connection."
    ///
    /// Java reads `trace.getLength()` off the item objects the set holds; the port resolves each
    /// id through the board, which is why this takes one where Java's method takes nothing.
    pub fn trace_length(&self, board: &Board) -> f64 {
        // :134-139.
        let mut result = 0.0;
        for current_item in &self.item_list {
            if let Some(Item::Trace(trace)) = board.get_item(*current_item) {
                result += trace.get_length();
            }
        }
        // :140.
        result
    }

    /// Port of `getDetour()` (Connection.java:143-154): "returns an estimation of the actual
    /// length of the connection divided by the minimal possible length."
    ///
    /// `Integer.MAX_VALUE` (`:149`) is a **`double`** here as it is there — the method's return
    /// type is `double`, so the `int` constant widens — and `MazeRipupResolver.java:164` divides
    /// by it rather than by an integer.
    pub fn get_detour(&self, board: &Board) -> f64 {
        // :148-150.
        let (Some(start_point), Some(end_point)) = (&self.start_point, &self.end_point) else {
            return f64::from(i32::MAX);
        };
        // :151.
        let min_trace_length = start_point.to_float().distance(&end_point.to_float());
        // :152-153.
        (self.trace_length(board) + DETOUR_ADD) / (min_trace_length + DETOUR_ADD)
            + DETOUR_ITEM_COST * (self.item_list.len() as f64 - 1.0)
    }
}

/// `Item.firstCommonLayer(Item)` as Java's `int`: `-1` for "no common layer".
fn layer_no(layer: Option<usize>) -> i32 {
    layer.map_or(-1, |l| i32::try_from(l).unwrap_or(i32::MAX))
}
