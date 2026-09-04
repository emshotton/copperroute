use std::collections::BTreeMap;

use fr_board::{Board, ItemId};

use crate::autoroute::AutorouteAttemptState;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RoutingFailureLog {
    failures: BTreeMap<ItemId, ItemFailureInfo>,
}

impl RoutingFailureLog {
    pub const FAILURE_THRESHOLD: i32 = 50;

    pub fn new() -> RoutingFailureLog {
        RoutingFailureLog::default()
    }

    pub fn record_failure(
        &mut self,
        board: &Board,
        item: ItemId,
        pass_no: i32,
        state: AutorouteAttemptState,
        reason: Option<&str>,
    ) {
        let info = self
            .failures
            .entry(item)
            .or_insert_with(|| ItemFailureInfo::new(board, item));
        info.record_failure(pass_no, state, reason);
    }

    pub fn failure_count(&self, item: ItemId) -> i32 {
        self.failures
            .get(&item)
            .map_or(0, |info| info.failure_count)
    }

    pub fn entry(&self, item: ItemId) -> Option<&ItemFailureInfo> {
        self.failures.get(&item)
    }

    pub fn len(&self) -> usize {
        self.failures.len()
    }

    pub fn is_empty(&self) -> bool {
        self.failures.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemFailureInfo {
    pub item: ItemId,
    pub net_number: i32,
    pub failure_count: i32,
    pub last_failure_state: Option<AutorouteAttemptState>,
    pub last_failure_reason: String,
    pub last_attempt_pass: i64,
}

impl ItemFailureInfo {
    fn new(board: &Board, item: ItemId) -> ItemFailureInfo {
        let net_number = board
            .get_item(item)
            .filter(|i| i.net_count() > 0)
            .map_or(-1, |i| i.get_net_number(0));
        ItemFailureInfo {
            item,
            net_number,
            failure_count: 0,
            last_failure_state: None,
            last_failure_reason: String::new(),
            last_attempt_pass: 0,
        }
    }

    fn record_failure(&mut self, pass_no: i32, state: AutorouteAttemptState, reason: Option<&str>) {
        self.failure_count += 1;
        self.last_attempt_pass = i64::from(pass_no);
        self.last_failure_state = Some(state);
        self.last_failure_reason = reason.unwrap_or("").to_string();
    }
}
