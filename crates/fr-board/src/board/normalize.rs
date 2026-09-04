use std::collections::BTreeMap;

use fr_geometry::{Point, TileShape};

use crate::datastructures::StopCheck;
use crate::error::BoardError;
use crate::ids::ItemId;
use crate::items::Item;

use super::Board;

pub const MAX_NORMALIZE_ITERATIONS: u32 = 2000;

impl Board {
                pub fn combine_traces(&mut self, net_number: i32) -> Result<bool, BoardError> {
        let mut result = false;
        let mut something_changed = true;
        while something_changed {
            something_changed = false;
            for id in self.items_in_board_order() {
                let Some(item) = self.items.get(&id) else {
                    continue;
                };
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

                                            pub fn normalize_traces(&mut self, net_number: i32) -> Result<bool, BoardError> {
        self.normalize_traces_checked(net_number, &|| false)
    }

                pub fn normalize_traces_checked(
        &mut self,
        net_number: i32,
        stop: StopCheck<'_>,
    ) -> Result<bool, BoardError> {
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
            if iteration_count > MAX_NORMALIZE_ITERATIONS {
                self.normalize_suppressed_net_nos.insert(net_number);
                break;
            }
            something_changed = false;
            let net_traces: Vec<ItemId> = self
                .items_in_board_order()
                .into_iter()
                .filter(|id| {
                    self.items.get(id).is_some_and(|item| {
                        item.contains_net(net_number) && item.is_trace() && item.is_on_the_board()
                    })
                })
                .collect();
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

                                                pub fn normalize_all_traces(&mut self) -> Result<bool, BoardError> {
        self.normalize_all_traces_checked(&|| false)
    }

                        pub fn normalize_all_traces_checked(
        &mut self,
        stop: StopCheck<'_>,
    ) -> Result<bool, BoardError> {
        let mut result = false;
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

        for (net_number, initial_traces) in traces_by_net {
            let mut net_traces = initial_traces;
            let mut something_changed = true;
            let mut iteration_count: u32 = 0;
            while something_changed {
                if stop() {
                    return Err(BoardError::Stopped);
                }
                iteration_count += 1;
                if iteration_count > MAX_NORMALIZE_ITERATIONS {
                    break;
                }
                something_changed = false;
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

                pub fn split_traces(
        &mut self,
        location: &Point,
        layer: usize,
        net_number: i32,
    ) -> Result<bool, BoardError> {
        self.split_traces_checked(location, layer, net_number, &|| false)
    }

                                pub fn split_traces_checked(
        &mut self,
        location: &Point,
        layer: usize,
        net_number: i32,
        stop: StopCheck<'_>,
    ) -> Result<bool, BoardError> {
        let picked = self.pick_traces(location, Some(layer));
        let location_shape = TileShape::get_instance_from_point(location).bounding_octagon();
        let mut trace_split = false;
        for id in picked.into_iter().rev() {
            if !self
                .items
                .get(&id)
                .is_some_and(|item| item.contains_net(net_number))
            {
                continue;
            }
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
