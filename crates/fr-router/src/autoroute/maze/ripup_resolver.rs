//! Port of `autoroute.maze.MazeRipupResolver` (MazeRipupResolver.java:25-269) — "resolves whether
//! maze expansion may rip up an obstacle and calculates its cost".
//!
//! # Shape and visibility
//!
//! Like [`MazeExpansionEngine`](super::MazeExpansionEngine), Java's class is `final`,
//! package-private and holds one `MazeSearchEngine` field its constructor stores (`:31-33`); the
//! port is a unit struct whose associated functions take that field as their leading parameter.
//! `enterThroughSmallDoor` is `private` in Java and `pub` here for the reason `search.rs` gives
//! for its four: the ground truth is a same-package probe with `setAccessible(true)`, and neither
//! of its two `false` arms — a door that is not 1-dimensional, and a neighbourhood holding an
//! item that neither *is* nor *contacts* the ignored one — is separable through `checkRipup`.
//!
//! # The one randomness source in Plan 6
//!
//! `:158-163`: from ripup pass 4 onwards, and on every pass whose number is not a multiple of 3,
//! the detour is scaled by `0.5 + r²` for one `nextDouble()` off the search's own `Random`. That
//! generator is seeded with `ctrl.ripupCosts` at `MazeSearchEngine.java:79-80` (plan-6 ruling 5)
//! and **this is its only draw anywhere in the plan**, so the whole search's randomness is the
//! sequence of `checkRipup` calls that reach `:160`.

use fr_board::{Board, FixedState, Item, ItemId, TreeObject};
use fr_geometry::{Line, Polyline, java_max};

use crate::autoroute::expansion::{ExpandableRef, ObstacleExpansionRoom, RoomRef};
use crate::autoroute::maze::engine::tree_of;
use crate::autoroute::maze::{
    ALREADY_RIPPED_COSTS, MazeAdjustment, MazeListElement, MazeSearchEngine, TRACE_WIDTH_TOLERANCE,
};
use crate::autoroute::path::Connection;

/// `private static final double FANOUT_COST_CONSTANT = 20000` (MazeRipupResolver.java:27).
const FANOUT_COST_CONSTANT: f64 = 20000.0;

/// Port of `maze.MazeRipupResolver` (MazeRipupResolver.java:25-269).
pub struct MazeRipupResolver;

impl MazeRipupResolver {
    // =============================================================================================
    // calcFanoutViaRipupCostFactor (:35-66)
    // =============================================================================================

    /// Port of the static `calcFanoutViaRipupCostFactor(Trace)` (MazeRipupResolver.java:35-66):
    /// the multiplier that protects a fanout via's escape trace from being ripped.
    ///
    /// A trace qualifies when **one of its two ends** has exactly one contact and that contact is
    /// either an SMD pin (a `Pin` living on a single layer, `:48-50`) or a `SHOVE_FIXED`
    /// two-corner `PolylineTrace` (`:51-56`). The factor is then
    /// `max(20000 · (halfWidth / length)², 1)` — the shorter and thinner the escape, the dearer.
    ///
    /// Java's method takes the `Trace` object; the port takes its id and the board, because the
    /// two contact sets come off `Board`.
    ///
    /// `1.0` for an item that is not a trace on the board: Java's parameter is typed `Trace` and
    /// both call sites (`:103`, `:116`) have already pattern-matched one.
    pub fn calc_fanout_via_ripup_cost_factor(board: &Board, trace: ItemId) -> f64 {
        let Some(Item::Trace(obstacle_trace)) = board.get_item(trace) else {
            return 1.0;
        };
        let half_width = f64::from(obstacle_trace.get_half_width());
        let length = obstacle_trace.get_length();
        // :37-42.
        for end in 0..2 {
            let current_end_contacts = if end == 0 {
                board.trace_start_contacts(trace)
            } else {
                board.trace_end_contacts(trace)
            };
            // :43-45.
            if current_end_contacts.len() != 1 {
                continue;
            }
            // :46. A one-element `TreeSet`; the port's `BTreeSet` answers the same element.
            let current_trace_contact = *current_end_contacts
                .iter()
                .next()
                .expect("the length is exactly one");
            // :47-56.
            let mut protect_fanout_via = false;
            if let Some(contact) = board.get_item(current_trace_contact) {
                let ctx = board.ctx();
                match contact {
                    // :48-50.
                    Item::Pin(_) => {
                        if contact.first_layer(&ctx) == contact.last_layer(&ctx) {
                            protect_fanout_via = true;
                        }
                    }
                    // :51-56.
                    Item::Trace(contact_trace) => {
                        if contact.get_fixed_state() == FixedState::ShoveFixed
                            && contact_trace.corner_count() == 2
                        {
                            protect_fanout_via = true;
                        }
                    }
                    _ => {}
                }
            }
            // :58-63.
            if protect_fanout_via {
                let mut fanout_via_cost_factor = half_width / length;
                fanout_via_cost_factor *= fanout_via_cost_factor;
                fanout_via_cost_factor *= FANOUT_COST_CONSTANT;
                return java_max(fanout_via_cost_factor, 1.0);
            }
        }
        // :65.
        1.0
    }

    // =============================================================================================
    // checkRipup (:72-197)
    // =============================================================================================

    /// Port of `checkRipup(MazeListElement, Item, boolean)` (MazeRipupResolver.java:72-197):
    /// "checks whether the next room can be ripped and returns its cost, or -1 when it cannot be
    /// ripped."
    ///
    /// # The cost model in one place (`:96-168`)
    ///
    /// * `costFactor` is the obstacle trace's half width (`:101`), or for a via the **largest**
    ///   half width among its trace contacts scaled by `0.5 · (contactCount − 1)` (`:114`,
    ///   `:123-125`) — so a via with a single contact is free, and the `max(…, 1)` of `:166` is
    ///   what it ends up costing.
    /// * a via with any non-trace or user-fixed contact is **not rippable at all** (`:110-112`).
    /// * `ripupCost = ctrl.ripupCosts · costFactor`, then `/ detour · fanoutViaCostFactor`, then
    ///   clamped into `[1, Integer.MAX_VALUE / 100]` (`:166-168`).
    /// * `detour` is `Connection.get(obstacleItem).getDetour()` (`:135-137`) — but only when the
    ///   obstacle is **not** already fanout-protected and this is not a fanout pass (`:134`);
    ///   otherwise it stays `1` and the factor multiplies instead.
    ///
    /// The `FRLogger.trace` of `:173-195` and the four locals that exist only to fill it in
    /// (`traceLength`, `minTraceLength`, `itemCount`, `connectionItemIds`, `:130-133`,
    /// `:138-156`, `:169-172`) are dropped; the `Connection.get` they share with the model is not.
    pub fn check_ripup(
        search: &mut MazeSearchEngine<'_>,
        board: &mut Board,
        list_element: &MazeListElement,
        obstacle_item: ItemId,
        door_is_small: bool,
    ) -> i32 {
        // :74-76.
        if !board.get_item(obstacle_item).is_some_and(Item::is_routable) {
            return -1;
        }
        // :77-79.
        if door_is_small
            && !Self::enter_through_small_door(search, board, list_element, obstacle_item)
        {
            return -1;
        }
        // :80. `listElement.door` is an `ExpandableObject` and `nextRoom` a
        // `CompleteExpansionRoom`, so this is `ExpandableObject.otherRoom(CompleteExpansionRoom)`
        // — the narrowing overload, which answers `null` for an incomplete room on the far side.
        let previous_room = list_element
            .next_room
            .and_then(|room| search.engine.expandable_other_room(list_element.door, room));
        // :81.
        let room_was_shoved = list_element.adjustment != MazeAdjustment::None;
        // :82-85.
        let previous_item = match previous_room {
            Some(RoomRef::Obstacle(room)) => search
                .engine
                .rooms
                .obstacle_room(room)
                .map(ObstacleExpansionRoom::get_item),
            _ => None,
        };
        if room_was_shoved {
            // :86-91.
            if let Some(previous_item) = previous_item
                && previous_item != obstacle_item
                && shares_net(board, previous_item, obstacle_item)
            {
                return -1;
            }
        } else if previous_item == Some(obstacle_item) {
            // :92-94.
            return ALREADY_RIPPED_COSTS;
        }

        // :96-99.
        let mut fanout_via_cost_factor = 1.0f64;
        let mut cost_factor = 1.0f64;
        // `wrapping_mul`, not `saturating_mul`: Java's `* 2` is an `int` multiply that wraps.
        // `RouterSettings.setStartRipupCosts` clamps its argument to at least 1 and no DSN carries
        // a value near `Integer.MAX_VALUE / 2`, so the two agree on every reachable input.
        let preserve_fanout_protection = !search.ctrl.remove_unconnected_vias
            && search.ctrl.ripup_costs <= search.ctrl.start_ripup_costs.wrapping_mul(2);
        match board.get_item(obstacle_item) {
            // :100-104.
            Some(Item::Trace(obstacle_trace)) => {
                cost_factor = f64::from(obstacle_trace.get_half_width());
                if preserve_fanout_protection {
                    fanout_via_cost_factor =
                        Self::calc_fanout_via_ripup_cost_factor(board, obstacle_item);
                }
            }
            // :105-126.
            Some(Item::Via(_)) => {
                let mut look_if_fanout_via = preserve_fanout_protection;
                // :107. Java's `TreeSet<Item>` is **descending** id, so the walk is `.rev()`; the
                // order is observable through `lookIfFanoutVia`, which stops at the first contact
                // whose factor exceeds 1.
                let contact_list = board.normal_contacts(obstacle_item);
                let mut contact_count = 0i32;
                for current_contact in contact_list.into_iter().rev() {
                    // :110-112. Java's `!(currentContact instanceof Trace) || isUserFixed()`
                    // short-circuits in that order; a contact the board no longer holds is Java's
                    // `NullPointerException` and is unreachable from `getNormalContacts`.
                    let Some(contact_item) = board.get_item(current_contact) else {
                        return -1;
                    };
                    let Item::Trace(obstacle_trace) = contact_item else {
                        return -1;
                    };
                    if contact_item.is_user_fixed() {
                        return -1;
                    }
                    let contact_half_width = f64::from(obstacle_trace.get_half_width());
                    // :113-114.
                    contact_count += 1;
                    cost_factor = java_max(cost_factor, contact_half_width);
                    // :115-121.
                    if look_if_fanout_via && !search.ctrl.is_fanout {
                        let current_fanout_via_cost_factor =
                            Self::calc_fanout_via_ripup_cost_factor(board, current_contact);
                        if current_fanout_via_cost_factor > 1.0 {
                            fanout_via_cost_factor = current_fanout_via_cost_factor;
                            look_if_fanout_via = false;
                        }
                    }
                }
                // :123-125.
                if fanout_via_cost_factor <= 1.0 {
                    cost_factor *= 0.5 * f64::from((contact_count - 1).max(0));
                }
            }
            _ => {}
        }

        // :128-129.
        let mut ripup_cost = f64::from(search.ctrl.ripup_costs) * cost_factor;
        let mut detour = 1.0f64;
        // :134-157.
        if fanout_via_cost_factor <= 1.0 && !search.ctrl.is_fanout {
            // :135-137.
            if let Some(obstacle_connection) =
                Connection::get(board, &mut search.engine.connections, obstacle_item)
                && let Some(connection) = search.engine.connections.get(obstacle_connection.0)
            {
                detour = connection.get_detour(board);
            }
        }
        // :158-163. Plan 6's only randomness.
        let randomize = search.ctrl.ripup_pass_no >= 4 && search.ctrl.ripup_pass_no % 3 != 0;
        if randomize {
            let random_number = search.random_generator.next_double();
            let random_factor = 0.5 + random_number * random_number;
            detour *= random_factor;
        }
        // :164-165.
        ripup_cost /= detour;
        ripup_cost *= fanout_via_cost_factor;
        // :166. Java's `(int)` narrowing truncates toward zero and **saturates**, which is what
        // Rust's `as i32` does.
        let result = (ripup_cost as i32).max(1);
        // :167-168.
        let max_ripup_costs = i32::MAX / 100;
        result.min(max_ripup_costs)
    }

    // =============================================================================================
    // checkLeavingRippedItem (:200-213)
    // =============================================================================================

    /// Port of `checkLeavingRippedItem(MazeListElement)` (MazeRipupResolver.java:200-213):
    /// "checks entering a thick room from a via or trace through a small door after ripup."
    ///
    /// `MazeSearchEngine.java:506` reaches it whenever the popped door is small and the free-space
    /// room behind it is thick — which needs no `ctrl.ripupAllowed`, so it is on the hot path of
    /// every routing pass.
    ///
    /// `currentDoor` is statically an `ExpansionDoor` at `:204`, so the `otherRoom` call is the
    /// **narrowing** overload (ExpansionDoor.java:78-92), which answers `null` for an incomplete
    /// room on the far side.
    pub fn check_leaving_ripped_item(
        search: &mut MazeSearchEngine<'_>,
        board: &mut Board,
        list_element: &MazeListElement,
    ) -> bool {
        // :201-203.
        let ExpandableRef::Door(current_door) = list_element.door else {
            return false;
        };
        // :204-207.
        let Some(next_room) = list_element.next_room else {
            return false;
        };
        let from_room = search
            .engine
            .rooms
            .door(current_door)
            .and_then(|door| door.other_complete_room(next_room));
        let Some(RoomRef::Obstacle(obstacle_room)) = from_room else {
            return false;
        };
        // :208.
        let Some(current_item) = search
            .engine
            .rooms
            .obstacle_room(obstacle_room)
            .map(ObstacleExpansionRoom::get_item)
        else {
            return false;
        };
        // :209-211.
        if !board.get_item(current_item).is_some_and(Item::is_routable) {
            return false;
        }
        // :212.
        Self::enter_through_small_door(search, board, list_element, current_item)
    }

    // =============================================================================================
    // enterThroughSmallDoor (:219-268)
    // =============================================================================================

    /// Port of the private `enterThroughSmallDoor(MazeListElement, Item)`
    /// (MazeRipupResolver.java:219-268): "checks whether a door can be entered while ignoring the
    /// obstacle item and its directly connected items."
    ///
    /// It builds a three-line band across the door — the door line offset by
    /// `± (compensatedTraceHalfWidth + TRACE_WIDTH_TOLERANCE)`, closed by a line through the
    /// door's centre of gravity perpendicular to it (`:243-249`) — and answers `false` as soon as
    /// the band overlaps any item that is neither `ignoreItem` nor one of its normal contacts
    /// (`:256-266`).
    ///
    /// # The three `false`s that are not the loop
    ///
    /// `:220-222` a door that is not 1-dimensional (which is every `TargetItemExpansionDoor`,
    /// `ExpansionDrill` and `DrillPage`, all of dimension 2); `:235-237` a door shape whose
    /// corners are all within one unit of each other, so `:227-234` never found a border line.
    pub fn enter_through_small_door(
        search: &mut MazeSearchEngine<'_>,
        board: &mut Board,
        list_element: &MazeListElement,
        ignore_item: ItemId,
    ) -> bool {
        // :220-222.
        if Self::expandable_dimension(search, list_element.door) != 1 {
            return false;
        }
        // :223. Only an `ExpansionDoor` reaches here — the other three implementors answer 2.
        let ExpandableRef::Door(door) = list_element.door else {
            return false;
        };
        let Some(door_shape) = search.engine.rooms.door_shape(door) else {
            // Java NPEs on a door whose rooms have no shape.
            return false;
        };
        // :224-234. `cornerApprox` answers `null` only for a shape with **no** border lines
        // (Simplex.java:174-176), which Java then dereferences at `:229`; a door of dimension 1
        // always has some. Every index below is `< borderLineCount()`, which is exactly the range
        // `cornerApprox` does not clamp (Simplex.java:181-184).
        let mut door_line: Option<Line> = None;
        let Some(mut prev_corner) = door_shape.corner_approx(0) else {
            return false;
        };
        let corner_count = door_shape.border_line_count();
        for i in 1..corner_count {
            let Some(next_corner) = door_shape.corner_approx(i) else {
                break;
            };
            if next_corner.distance_square(&prev_corner) > 1.0 {
                door_line = door_shape.border_line(i - 1);
                break;
            }
            prev_corner = next_corner;
        }
        // :235-237.
        let Some(door_line) = door_line else {
            return false;
        };

        // :239-242.
        let door_centre = door_shape.centre_of_gravity().round();
        let Some(current_layer) = list_element
            .next_room
            .and_then(|room| search.engine.rooms.room_layer(board, room))
        else {
            return false;
        };
        let check_radius =
            search.ctrl.compensated_trace_half_width[current_layer] + TRACE_WIDTH_TOLERANCE;
        // :243-246.
        let lines = vec![
            door_line.translate(f64::from(check_radius)),
            Line::from_direction(door_centre, &door_line.direction().turn_45_degree(2)),
            door_line.translate(-f64::from(check_radius)),
        ];
        // :248-249.
        let Ok(check_polyline) = Polyline::from_lines(lines) else {
            // `Polyline`'s normalising constructor throws on exactly one input class; Java would
            // propagate that out of here.
            return false;
        };
        let Some(check_shape) = check_polyline.offset_shape(check_radius, 0) else {
            // Java NPEs at `:253` on a null shape.
            return false;
        };

        // :250-254.
        let ignore_net_nos = [search.ctrl.net_number];
        let overlapping_objects = {
            let ctx = board.ctx();
            tree_of(board, search.search_tree).overlapping_objects_with_rooms(
                &check_shape,
                Some(current_layer),
                &ignore_net_nos,
                &board.items,
                &search.engine.rooms,
                &ctx,
            )
        };

        // :256-266.
        for current_object in overlapping_objects {
            // "!(currentObject instanceof Item)" is the room arm; a room is skipped.
            let TreeObject::Item(current_item) = current_object else {
                continue;
            };
            if current_item == ignore_item {
                continue;
            }
            // :260-262.
            if !shares_net(board, current_item, ignore_item) {
                return false;
            }
            // :263-265.
            if !board.normal_contacts(current_item).contains(&ignore_item) {
                return false;
            }
        }
        // :267.
        true
    }

    /// `ExpandableObject.getDimension()` (ExpandableObject.java:13) over the four implementors —
    /// `MazeRipupResolver.java:220`'s virtual call. The three non-door implementors answer the
    /// constant 2 (TargetItemExpansionDoor.java:39-42, ExpansionDrill.java:99-102,
    /// DrillPage.java's twin).
    fn expandable_dimension(search: &MazeSearchEngine<'_>, object: ExpandableRef) -> i32 {
        match object {
            ExpandableRef::Door(door) => search
                .engine
                .rooms
                .door(door)
                .map_or(0, |door| door.dimension),
            ExpandableRef::TargetDoor(_) | ExpandableRef::Drill(_) | ExpandableRef::Page(_) => 2,
        }
    }
}

/// `Item.sharesNet(Item)` (Item.java) through two ids. `false` for an item the board no longer
/// holds, where Java would have NPE'd on a live reference.
fn shares_net(board: &Board, a: ItemId, b: ItemId) -> bool {
    match (board.get_item(a), board.get_item(b)) {
        (Some(a), Some(b)) => a.shares_net(b),
        _ => false,
    }
}

// =================================================================================================
// The deferral roster for `autoroute/maze/MazeRipupResolver.java`
// =================================================================================================

// not ported: `MazeRipupResolver.MazeRipupResolver` — the constructor, which stores the one
// `MazeSearchEngine` field. The port's functions take that field as their leading parameter
// (module docs), so there is nothing to construct.
//
// not ported: the `FRLogger.trace("CHECK_RIPUP …")` of `:173-195` and the four locals that exist
// only to fill it in — `traceLength` (`:130`), `minTraceLength` (`:131`), `itemCount` (`:132`) and
// `connectionItemIds` (`:133`, `:147-155`). Plan-6's global constraints drop the logger; the
// `Connection.get` those share with the cost model **is** ported, at `:135`.
