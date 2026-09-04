use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignRulesCheckerSettings {
                        #[serde(skip)]
    pub enabled: bool,

            #[serde(rename = "include_warnings")]
    pub include_warnings: bool,

            #[serde(rename = "include_errors")]
    pub include_errors: bool,
}

impl Default for DesignRulesCheckerSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            include_warnings: true,
            include_errors: true,
        }
    }
}

impl DesignRulesCheckerSettings {
        pub const FIELD_NAMES: &'static [&'static str] =
        &["enabled", "include_warnings", "include_errors"];
}


#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebugSettings {
        #[serde(rename = "enable_detailed_logging")]
    pub enable_detailed_logging: bool,

        #[serde(rename = "single_step_execution")]
    pub single_step_execution: bool,

        #[serde(rename = "trace_insertion_delay")]
    pub trace_insertion_delay: i32,

            #[serde(rename = "filter_by_net")]
    pub filter_by_net: HashSet<String>,

            #[serde(rename = "operation_filters")]
    pub operation_filters: Vec<String>,
}

impl Default for DebugSettings {
        fn default() -> Self {
        Self {
            enable_detailed_logging: false,
            single_step_execution: false,
            trace_insertion_delay: 0,
            filter_by_net: HashSet::new(),
            operation_filters: vec![
                "insert_trace_segment".to_string(),
                "remove_trace_segment".to_string(),
                "insert_trace_failure".to_string(),
                "remove_tail".to_string(),
                "insert_trace".to_string(),
                "remove_trace".to_string(),
                "insert_via".to_string(),
                "remove_via".to_string(),
            ],
        }
    }
}

impl DebugSettings {
        pub const FIELD_NAMES: &'static [&'static str] = &[
        "enable_detailed_logging",
        "single_step_execution",
        "trace_insertion_delay",
        "filter_by_net",
        "operation_filters",
    ];

            pub fn new() -> Self {
        Self::default()
    }

                                    pub fn is_net_permitted(&self, net_number: i32, net_name: Option<&str>) -> bool {
        if self.filter_by_net.is_empty() {
            return true;
        }
        let net_no_str = net_number.to_string();
        self.filter_by_net.contains(&net_no_str)
            || self.filter_by_net.contains(&format!("Net #{net_number}"))
            || self.filter_by_net.contains(&format!("Net#{net_number}"))
            || net_name.is_some_and(|name| self.filter_by_net.contains(&name.to_lowercase()))
    }
}
