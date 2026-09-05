use std::cmp::Ordering;

use fr_board::ItemId;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ItemRouteResult {
    item_id: ItemId,
    improvement_percentage: f32,
    via_count_before: i32,
    via_count_after: i32,
    trace_length_before: f64,
    trace_length_after: f64,
    incomplete_count_before: i32,
    incomplete_count_after: i32,
    improved: bool,
}

impl ItemRouteResult {
    pub fn unimproved(item_id: ItemId) -> ItemRouteResult {
        ItemRouteResult::new(item_id, 0, 0, 0.0, 0.0, 0, 1)
    }

    pub fn new(
        item_id: ItemId,
        via_count_before: i32,
        via_count_after: i32,
        trace_length_before: f64,
        trace_length_after: f64,
        incomplete_count_before: i32,
        incomplete_count_after: i32,
    ) -> ItemRouteResult {
        let improved = if incomplete_count_after < incomplete_count_before {
            true
        } else if incomplete_count_after > incomplete_count_before {
            false
        } else if via_count_after < via_count_before {
            true
        } else if via_count_after > via_count_before {
            false
        } else if trace_length_after < trace_length_before {
            true
        } else {
            false
        };

        let improvement_percentage = if via_count_before != 0 && trace_length_before != 0.0 {
            let via_term = f64::from(via_count_after / via_count_before);
            let length_term = trace_length_after / trace_length_before;
            (1.0 - ((via_term + length_term) / 2.0)) as f32
        } else {
            0.0
        };

        ItemRouteResult {
            item_id,
            improvement_percentage,
            via_count_before,
            via_count_after,
            trace_length_before,
            trace_length_after,
            incomplete_count_before,
            incomplete_count_after,
            improved,
        }
    }

    pub fn compare_to(&self, r: &ItemRouteResult) -> Ordering {
        if self.incomplete_count_after < r.incomplete_count_after {
            Ordering::Less
        } else if self.incomplete_count_after > r.incomplete_count_after {
            Ordering::Greater
        } else if self.via_count_after < r.via_count_after {
            Ordering::Less
        } else if self.via_count_after > r.via_count_after {
            Ordering::Greater
        } else if self.trace_length_after < r.trace_length_after {
            Ordering::Less
        } else if self.trace_length_after > r.trace_length_after {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }

    pub fn improved_over(&self, r: &ItemRouteResult) -> bool {
        self.compare_to(r) == Ordering::Less
    }

    pub fn item_id(&self) -> ItemId {
        self.item_id
    }

    pub fn improved(&self) -> bool {
        self.improved
    }

    pub fn improvement_percentage(&self) -> f32 {
        self.improvement_percentage
    }

    pub fn via_count(&self) -> i32 {
        self.via_count_after
    }

    pub fn trace_length(&self) -> f64 {
        self.trace_length_after
    }

    pub fn incomplete_count(&self) -> i32 {
        self.incomplete_count_after
    }

    pub fn via_count_reduced(&self) -> i32 {
        self.via_count_before.wrapping_sub(self.via_count_after)
    }

    pub fn length_reduced(&self) -> f64 {
        self.trace_length_before - self.trace_length_after
    }

    pub fn update_improved(&mut self, improved: bool) {
        self.improved = improved;
    }

    pub fn incomplete_count_before(&self) -> i32 {
        self.incomplete_count_before
    }
}
