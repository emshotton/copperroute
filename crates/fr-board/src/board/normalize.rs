//! `BasicBoard`'s four normalisation loops: `combineTraces` (BasicBoard.java:683-706),
//! `normalizeTraces` (:709-795), `normalizeAllTraces` (:798-885) and `splitTraces` (:891-907),
//! plus the `normalizeSuppressedNetNos` bookkeeping the second of them keeps (:96,710-747).
//!
//! Each is a fixed-point loop over the item list, and each walks it in **descending item id**
//! (quirk #63), which [`Board::items_in_board_order`] produces.

use std::collections::BTreeMap;

use fr_geometry::{Point, TileShape};

use crate::datastructures::StopCheck;
use crate::error::BoardError;
use crate::ids::ItemId;
use crate::items::Item;

use super::Board;

/// Java `BasicBoard.MAX_NORMALIZE_ITERATIONS` (BasicBoard.java:64): the safety valve on
/// [`Board::normalize_traces`]' outer loop. Past it the net is almost certainly oscillating
/// (split → combine → split …).
pub const MAX_NORMALIZE_ITERATIONS: u32 = 2000;

impl Board {
    /// Port of `BasicBoard.combineTraces(int)` (BasicBoard.java:683-706): combine the connected
    /// traces of this net that have only one contact at the connection point, until nothing more
    /// combines. `net_number < 0` combines the traces of every net.
    pub fn combine_traces(&mut self, net_number: i32) -> Result<bool, BoardError> {
        let mut result = false;
        let mut something_changed = true;
        while something_changed {
            something_changed = false;
            // BasicBoard.java:687-705, over the item list in board order.
            for id in self.items_in_board_order() {
                let Some(item) = self.items.get(&id) else {
                    continue;
                };
                // BasicBoard.java:693-695.
                if !((net_number < 0 || item.contains_net(net_number))
                    && item.is_trace()
                    && item.is_on_the_board())
                {
                    continue;
                }
                if self.combine_trace(id)? {
                    something_changed = true;
                    result = true;
                    break;
                }
            }
        }
        Ok(result)
    }

    /// Port of `BasicBoard.normalizeTraces(int)` (BasicBoard.java:709-795): normalise every trace
    /// of this net until the board stops changing, or until
    /// [`MAX_NORMALIZE_ITERATIONS`] passes have gone by — after which the net is added to
    /// [`Board::normalize_suppressed_net_nos`] and every later call for it answers `false`
    /// without doing any work.
    ///
    /// Java's `ConcurrentModificationException` retry (:757-763) has no counterpart: the port
    /// collects the ids from a `BTreeMap` it is not iterating while it mutates, so the
    /// collection pass cannot fail. Its effect — `somethingChanged = true; continue;` — is a
    /// re-collection, which is what the next pass does anyway.
    // not ported: the `FRLogger.debug`/`FRLogger.warn` messages at BasicBoard.java:718-726 and
    // :737-746 (and the `netName` lookups that only feed them).
    pub fn normalize_traces(&mut self, net_number: i32) -> Result<bool, BoardError> {
        self.normalize_traces_checked(net_number, &|| false)
    }

    /// [`Board::normalize_traces`] under a [`StopCheck`], consulted once per pass of the outer
    /// loop (Plan 3 ruling 4). A trip answers [`BoardError::Stopped`]; the board is left
    /// part-normalised, exactly as Java's would be if its `normalizeAllTraces` threw.
    // added in Plan 3: BasicBoard.normalizeTraces (plan ruling 4)
    pub fn normalize_traces_checked(
        &mut self,
        net_number: i32,
        stop: StopCheck<'_>,
    ) -> Result<bool, BoardError> {
        // BasicBoard.java:713-727.
        if self.normalize_suppressed_net_nos.contains(&net_number) {
            return Ok(false);
        }
        let mut result = false;
        let mut something_changed = true;
        let mut iteration_count: u32 = 0;
        while something_changed {
            if stop() {
                return Err(BoardError::Stopped);
            }
            iteration_count += 1;
            // BasicBoard.java:728-749.
            if iteration_count > MAX_NORMALIZE_ITERATIONS {
                self.normalize_suppressed_net_nos.insert(net_number);
                break;
            }
            something_changed = false;
            // BasicBoard.java:753-777.
            let net_traces: Vec<ItemId> = self
                .items_in_board_order()
                .into_iter()
                .filter(|id| {
                    self.items.get(id).is_some_and(|item| {
                        item.contains_net(net_number) && item.is_trace() && item.is_on_the_board()
                    })
                })
                .collect();
            // BasicBoard.java:783-792. The two arms are kept separate, as Java writes them:
            // `removeIfCycle` must not run when `normalize` already reported a change.
            #[allow(clippy::if_same_then_else)]
            for id in net_traces {
                if !self.items.get(&id).is_some_and(Item::is_on_the_board) {
                    continue;
                }
                if self.normalize_trace_checked(id, None, stop)? {
                    something_changed = true;
                    result = true;
                } else if !self.items.get(&id).is_some_and(Item::is_user_fixed)
                    && self.remove_if_cycle_checked(id, stop)?
                {
                    something_changed = true;
                    result = true;
                }
            }
        }
        Ok(result)
    }

    /// Port of `BasicBoard.normalizeAllTraces()` (BasicBoard.java:798-885): the same loop per
    /// net, over a map of the board's traces grouped by net number, built once up front and
    /// re-collected per net whenever a pass changed something.
    ///
    /// Java's map is a `HashMap<Integer, …>` and it walks `entrySet()`. For the small
    /// non-negative net numbers a board actually uses, a default-capacity `HashMap` buckets
    /// `Integer` keys by their own value, so that walk is ascending; the port uses a
    /// `BTreeMap`, which is ascending by construction (docs/java-quirks.md).
    ///
    /// Note this loop does **not** consult or update
    /// [`Board::normalize_suppressed_net_nos`] — only [`Board::normalize_traces`] does.
    pub fn normalize_all_traces(&mut self) -> Result<bool, BoardError> {
        self.normalize_all_traces_checked(&|| false)
    }

    /// [`Board::normalize_all_traces`] under a [`StopCheck`], consulted once per pass of each
    /// net's inner loop (Plan 3 ruling 4). This is the entry point `Wiring.readScope`'s
    /// `board.normalizeAllTraces()` (Wiring.java:347) becomes in the port; a trip answers
    /// [`BoardError::Stopped`], which `fr-dsn` turns into Java's own
    /// `"Wiring: normalization of traces failed"` warning.
    // added in Plan 3: BasicBoard.normalizeAllTraces (plan ruling 4)
    pub fn normalize_all_traces_checked(
        &mut self,
        stop: StopCheck<'_>,
    ) -> Result<bool, BoardError> {
        let mut result = false;
        // BasicBoard.java:801-826.
        let mut traces_by_net: BTreeMap<i32, Vec<ItemId>> = BTreeMap::new();
        for id in self.items_in_board_order() {
            let Some(item) = self.items.get(&id) else {
                continue;
            };
            if !(item.is_trace() && item.is_on_the_board()) {
                continue;
            }
            for net_number in item.net_nos() {
                traces_by_net.entry(*net_number).or_default().push(id);
            }
        }

        // BasicBoard.java:828-883.
        for (net_number, initial_traces) in traces_by_net {
            let mut net_traces = initial_traces;
            let mut something_changed = true;
            let mut iteration_count: u32 = 0;
            while something_changed {
                if stop() {
                    return Err(BoardError::Stopped);
                }
                iteration_count += 1;
                // BasicBoard.java:832-843.
                if iteration_count > MAX_NORMALIZE_ITERATIONS {
                    break;
                }
                something_changed = false;
                // BasicBoard.java:846-858; the two arms are separate for the same reason as
                // `normalize_traces`'.
                #[allow(clippy::if_same_then_else)]
                for id in net_traces.clone() {
                    if !self.items.get(&id).is_some_and(Item::is_on_the_board) {
                        continue;
                    }
                    if self.normalize_trace_checked(id, None, stop)? {
                        something_changed = true;
                        result = true;
                    } else if !self.items.get(&id).is_some_and(Item::is_user_fixed)
                        && self.remove_if_cycle_checked(id, stop)?
                    {
                        something_changed = true;
                        result = true;
                    }
                }
                // BasicBoard.java:860-881: re-collect this net's traces if anything changed.
                if something_changed {
                    net_traces = self
                        .items_in_board_order()
                        .into_iter()
                        .filter(|id| {
                            self.items.get(id).is_some_and(|item| {
                                item.contains_net(net_number)
                                    && item.is_trace()
                                    && item.is_on_the_board()
                            })
                        })
                        .collect();
                }
            }
        }
        Ok(result)
    }

    /// Port of `BasicBoard.splitTraces(Point, int, int)` (BasicBoard.java:891-907): split every
    /// trace of `net_number` on `layer` whose polygon contains `location`, restricting each split
    /// to the octagon around that point.
    pub fn split_traces(
        &mut self,
        location: &Point,
        layer: usize,
        net_number: i32,
    ) -> Result<bool, BoardError> {
        self.split_traces_checked(location, layer, net_number, &|| false)
    }

    /// [`Board::split_traces`] under a [`StopCheck`], threaded into every
    /// [`Board::split_trace_checked`] below it (plan-6 ruling 6, closing plan-3 ruling F). A trip
    /// answers [`BoardError::Stopped`]; the board is left part-split.
    ///
    /// The check is **not** consulted once per picked trace here: `split_trace_checked` already
    /// consults it inside the entry walk that does not terminate, and adding a second site would
    /// make a run stop earlier than the one Java's loop shape implies.
    // renamed: `BasicBoard.splitTraces` under a `StopCheck` -> `Board::split_traces_checked` (plan-6 ruling 6, closing plan-3 ruling F; the unchecked `Board::split_traces` above delegates to it with `|| false`).
    pub fn split_traces_checked(
        &mut self,
        location: &Point,
        layer: usize,
        net_number: i32,
        stop: StopCheck<'_>,
    ) -> Result<bool, BoardError> {
        // BasicBoard.java:892-895.
        let picked = self.pick_traces(location, Some(layer));
        let location_shape = TileShape::get_instance_from_point(location).bounding_octagon();
        let mut trace_split = false;
        // Java's `pickItems` answers a `TreeSet<Item>`: descending id (quirk #44).
        for id in picked.into_iter().rev() {
            // A trace an earlier split already removed: Java still calls `split` on the dead
            // object; see the totalization on `Board::split_trace`.
            if !self
                .items
                .get(&id)
                .is_some_and(|item| item.contains_net(net_number))
            {
                continue;
            }
            // BasicBoard.java:899-904.
            if self
                .split_trace_checked(id, Some(&location_shape), stop)?
                .len()
                != 1
            {
                trace_split = true;
            }
        }
        Ok(trace_split)
    }
}
