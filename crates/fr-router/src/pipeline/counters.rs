#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RouterCounters {
                            pub pass_count: Option<i32>,
        pub queued_to_be_routed_count: Option<i32>,
        pub routed_count: Option<i32>,
        pub skipped_count: Option<i32>,
        pub ripped_count: Option<i32>,
        pub failed_to_be_routed_count: Option<i32>,
        pub incomplete_count: Option<i32>,
        pub phase: Option<String>,
        pub fanout_extra_vias_count: Option<i32>,
}

impl RouterCounters {
                pub const JAVA_FIELD_NAMES: [&'static str; 9] = [
        "passCount",
        "queuedToBeRoutedCount",
        "routedCount",
        "skippedCount",
        "rippedCount",
        "failedToBeRoutedCount",
        "incompleteCount",
        "phase",
        "fanoutExtraViasCount",
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_counter_set_is_all_null() {
        let counters = RouterCounters::default();
        assert_eq!(counters.pass_count, None);
        assert_eq!(counters.phase, None);
        assert_eq!(counters.fanout_extra_vias_count, None);
    }

    #[test]
    fn the_java_field_name_list_has_nine_entries() {
        assert_eq!(RouterCounters::JAVA_FIELD_NAMES.len(), 9);
    }
}
