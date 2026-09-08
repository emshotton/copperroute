use std::collections::{BTreeMap, BTreeSet};

use copper_board::{Board, ItemId, ItemKind};

use crate::DrcViolation;
use crate::airline::AirLine;
use crate::net_incompletes::NetIncompletes;
use crate::unconnected::{UnconnectedItems, UnconnectedKind};

#[derive(Debug)]
pub struct DesignRulesChecker<'a> {
    pub(crate) board: &'a mut Board,
    max_connections: i32,
    net_incompletes: Option<Vec<NetIncompletes>>,
}

impl<'a> DesignRulesChecker<'a> {
    pub fn new(board: &'a mut Board) -> Self {
        DesignRulesChecker {
            board,
            max_connections: 0,
            net_incompletes: None,
        }
    }

    pub fn get_all_violations(&mut self) -> Vec<DrcViolation> {
        let constraints = crate::constraints::resolve(self.board);
        crate::checks::run_all(self.board, &constraints)
    }

    pub fn get_all_unconnected_items(&mut self) -> Vec<UnconnectedItems> {
        let mut unconnected_items: Vec<UnconnectedItems> = Vec::new();

        let mut items_by_net: BTreeMap<i32, Vec<ItemId>> = BTreeMap::new();
        for item in self.board.get_items() {
            if item.is_connectable() {
                items_by_net
                    .entry(item.get_net_number(0))
                    .or_default()
                    .push(item.id());
            }
        }

        for (&net_number, net_items) in &items_by_net {
            if net_items.len() <= 1 {
                continue;
            }

            let mut connected_sets: Vec<Vec<ItemId>> = Vec::new();
            let mut processed_items: BTreeSet<ItemId> = BTreeSet::new();
            let net_item_set: BTreeSet<ItemId> = net_items.iter().copied().collect();

            for &item in net_items {
                if processed_items.contains(&item) {
                    continue;
                }
                let set_items: Vec<ItemId> = self
                    .board
                    .connected_set(item, net_number, false)
                    .into_iter()
                    .filter(|id| net_item_set.contains(id))
                    .collect();

                if !set_items.is_empty() {
                    processed_items.extend(set_items.iter().copied());
                    connected_sets.push(set_items);
                }
            }

            if connected_sets.len() >= 2 {
                let item1 = self.find_representative_item(&connected_sets[0]);
                let item2 = self.find_representative_item(&connected_sets[1]);

                if let (Some(item1), Some(item2)) = (item1, item2) {
                    let mut group_items = connected_sets[0].clone();
                    group_items.extend_from_slice(&connected_sets[1]);
                    unconnected_items.push(UnconnectedItems::new_with_all_items(
                        item1,
                        item2,
                        group_items,
                    ));
                }
            }
        }

        for id in self.board.items_in_board_order() {
            let Some(item) = self.board.get_item(id) else {
                continue;
            };
            if !item.is_trace() {
                continue;
            }
            let start_is_empty = self.board.trace_start_contacts(id).is_empty();
            let end_is_empty = self.board.trace_end_contacts(id).is_empty();
            if !(start_is_empty || end_is_empty) {
                continue;
            }
            if unconnected_items.iter().any(|ui| ui.first_item == id) {
                continue;
            }
            unconnected_items.push(UnconnectedItems::new_typed(
                id,
                None,
                UnconnectedKind::TrackDangling,
            ));
        }

        for id in self.board.items_in_board_order() {
            if self.board.get_item(id).map(|item| item.kind()) != Some(ItemKind::Via) {
                continue;
            }
            if self.board.is_tail(id) {
                unconnected_items.push(UnconnectedItems::new_typed(
                    id,
                    None,
                    UnconnectedKind::ViaDangling,
                ));
            }
        }

        unconnected_items
    }

    fn find_representative_item(&self, connected_set: &[ItemId]) -> Option<ItemId> {
        let of_kind = |kind: ItemKind| {
            connected_set
                .iter()
                .copied()
                .find(|&id| self.board.get_item(id).map(|item| item.kind()) == Some(kind))
        };
        of_kind(ItemKind::Pin)
            .or_else(|| of_kind(ItemKind::Trace))
            .or_else(|| connected_set.first().copied())
    }

    pub fn calculate_all_incompletes(&mut self) {
        let board = &*self.board;
        let lists = crate::net_incompletes::net_item_lists(board);
        self.max_connections = crate::net_incompletes::max_connections(board, &lists);
        self.net_incompletes = Some(
            lists
                .iter()
                .enumerate()
                .map(|(i, items)| NetIncompletes::new(i as i32 + 1, items, board))
                .collect(),
        );
    }

    /// The incomplete count of just `nets`, built from the same per-net item lists
    /// [`DesignRulesChecker::calculate_all_incompletes`] builds, so it equals that pass's count
    /// for those nets.
    pub fn incomplete_count_for_nets(board: &Board, nets: &BTreeSet<i32>) -> usize {
        let mut net_item_lists: BTreeMap<i32, Vec<ItemId>> = nets
            .iter()
            .map(|net_number| (*net_number, Vec::new()))
            .collect();
        for id in board.items_in_board_order() {
            let Some(item) = board.get_item(id) else {
                continue;
            };
            if !item.is_connectable() {
                continue;
            }
            for i in 0..item.net_count() {
                if let Some(list) = net_item_lists.get_mut(&item.get_net_number(i)) {
                    list.push(id);
                }
            }
        }
        net_item_lists
            .iter()
            .map(|(net_number, items)| NetIncompletes::new(*net_number, items, board).count())
            .sum()
    }

    pub fn recalculate_net_incompletes(&mut self, net_number: i32) {
        if self.net_incompletes.is_none() {
            self.calculate_all_incompletes();
            return;
        }
        let board = &*self.board;
        let list = self.net_incompletes.as_mut().expect("just initialised");
        if let Some(index) = slot(net_number, list.len()) {
            let item_list = board.get_connectable_items(net_number);
            list[index] = NetIncompletes::new(net_number, &item_list, board);
        }
    }

    pub fn recalculate_net_incompletes_with(&mut self, net_number: i32, item_list: &[ItemId]) {
        if self.net_incompletes.is_none() {
            self.calculate_all_incompletes();
        }
        let board = &*self.board;
        let list = self.net_incompletes.as_mut().expect("just initialised");
        if let Some(index) = slot(net_number, list.len()) {
            list[index] = NetIncompletes::new(net_number, item_list, board);
        }
    }

    pub fn max_connections(&self) -> i32 {
        self.max_connections
    }

    pub fn get_incomplete_count(&mut self) -> usize {
        self.net_incompletes_mut()
            .iter()
            .map(NetIncompletes::count)
            .sum()
    }

    pub fn get_incomplete_count_for_net(&mut self, net_number: i32) -> usize {
        let list = self.net_incompletes_mut();
        match slot(net_number, list.len()) {
            Some(index) => list[index].count(),
            None => 0,
        }
    }

    pub fn get_length_violation_count(&mut self) -> usize {
        self.net_incompletes_mut()
            .iter()
            .filter(|net_incompletes| net_incompletes.get_length_violation() != 0.0)
            .count()
    }

    pub fn get_length_violation(&mut self, net_number: i32) -> f64 {
        let list = self.net_incompletes_mut();
        match slot(net_number, list.len()) {
            Some(index) => list[index].get_length_violation(),
            None => 0.0,
        }
    }

    pub fn recalculate_length_violations(&mut self) -> bool {
        if self.net_incompletes.is_none() {
            self.calculate_all_incompletes();
            return true;
        }
        let board = &*self.board;
        self.net_incompletes
            .as_mut()
            .expect("just checked")
            .iter_mut()
            .fold(false, |result, net_incompletes| {
                net_incompletes.calc_length_violation(board) || result
            })
    }

    pub fn get_all_airlines(&mut self) -> Vec<AirLine> {
        self.net_incompletes_mut()
            .iter()
            .flat_map(|net_incompletes| net_incompletes.incompletes.iter().cloned())
            .collect()
    }

    pub fn get_net_incompletes(&mut self, net_number: i32) -> Option<&NetIncompletes> {
        let list = self.net_incompletes_mut();
        let index = slot(net_number, list.len())?;
        Some(&list[index])
    }

    fn net_incompletes_mut(&mut self) -> &mut Vec<NetIncompletes> {
        if self.net_incompletes.is_none() {
            self.calculate_all_incompletes();
        }
        self.net_incompletes
            .as_mut()
            .expect("calculate_all_incompletes always assigns")
    }
}

fn slot(net_number: i32, len: usize) -> Option<usize> {
    if net_number <= 0 || net_number as usize > len {
        return None;
    }
    Some(net_number as usize - 1)
}
