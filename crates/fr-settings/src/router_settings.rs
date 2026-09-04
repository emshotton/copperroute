//! - `max_items`, `save_intermediate_stages`, `ignore_net_classes` get `#[serde(skip)]` — full
//! - `layers` gets `#[serde(skip_serializing)]` only — deserialisation stays enabled, matching
use serde::{Deserialize, Serialize};

use crate::{FanoutSettings, HostEnvironment, LayerSettings, OptimizerSettings, ScoringSettings};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct RouterSettings {
        #[serde(rename = "enabled", default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,

        #[serde(rename = "algorithm", default, skip_serializing_if = "Option::is_none")]
    pub algorithm: Option<String>,

        #[serde(
        rename = "fanout",
        default = "crate::json::constructed_fanout",
        skip_serializing_if = "Option::is_none"
    )]
    pub fanout: Option<FanoutSettings>,

        #[serde(
        rename = "copper_to_edge_clearance_um",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub copper_to_edge_clearance_um: Option<f64>,

        #[serde(
        rename = "hole_clearance_um",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub hole_clearance_um: Option<f64>,

                #[serde(
        rename = "neck_width_um",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub neck_width_um: Option<f64>,

                #[serde(
        rename = "strict_drc",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub strict_drc: Option<bool>,

        #[serde(
        rename = "job_timeout",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub job_timeout_string: Option<String>,

        #[serde(
        rename = "max_passes",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub max_passes: Option<i32>,

            #[serde(skip)]
    pub max_items: Option<i32>,

            #[serde(rename = "layers", default, skip_serializing)]
    pub layers: Option<Vec<LayerSettings>>,

            #[serde(skip)]
    pub save_intermediate_stages: Option<bool>,

            #[serde(skip)]
    pub ignore_net_classes: Option<Vec<String>>,

        #[serde(
        rename = "trace_pull_tight_accuracy",
        alias = "tracePullTightAccuracy",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub trace_pull_tight_accuracy: Option<i32>,

                        #[serde(
        rename = "allowed_via_types",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub vias_allowed: Option<bool>,

            #[serde(
        rename = "automatic_neckdown",
        alias = "automaticNeckdown",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub automatic_neckdown: Option<bool>,

        #[serde(
        rename = "optimizer",
        default = "crate::json::constructed_optimizer",
        skip_serializing_if = "Option::is_none"
    )]
    pub optimizer: Option<OptimizerSettings>,

        #[serde(
        rename = "scoring",
        default = "crate::json::constructed_scoring",
        skip_serializing_if = "Option::is_none"
    )]
    pub scoring: Option<ScoringSettings>,

        #[serde(
        rename = "max_threads",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub max_threads: Option<i32>,

            #[serde(
        rename = "result_json",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub result_json_path: Option<String>,

                            #[serde(skip)]
    pub(crate) board_specific_trace_costs_applied: Option<bool>,

                                                            #[serde(
        rename = "opt_changed_area_ms",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub opt_changed_area_ms: Option<i32>,
}

impl RouterSettings {
                                        pub const FIELD_NAMES: &'static [&'static str] = &[
        "enabled",
        "algorithm",
        "fanout",
        "copper_to_edge_clearance_um",
        "hole_clearance_um",
        "neck_width_um",
        "strict_drc",
        "job_timeout_string",
        "max_passes",
        "max_items",
        "layers",
        "save_intermediate_stages",
        "ignore_net_classes",
        "trace_pull_tight_accuracy",
        "vias_allowed",
        "automatic_neckdown",
        "optimizer",
        "scoring",
        "max_threads",
        "result_json_path",
        "board_specific_trace_costs_applied",
        "opt_changed_area_ms",
    ];

                            pub fn new() -> Self {
        Self {
            fanout: Some(FanoutSettings::default()),
            optimizer: Some(OptimizerSettings::default()),
            scoring: Some(ScoringSettings::default()),
            ..Default::default()
        }
    }

                            pub fn get_layer_count(&self) -> usize {
        self.layers.as_ref().map_or(0, Vec::len)
    }

                                                                                                    pub fn set_layer_count(&mut self, layer_count: usize) {
        if !matches!(&self.layers, Some(layers) if layers.len() == layer_count) {
            self.board_specific_trace_costs_applied = Some(false);
            self.layers = Some(vec![LayerSettings::default(); layer_count]);
        }
        let scoring = self.scoring.get_or_insert_with(ScoringSettings::default);
        scoring.preferred_direction_trace_cost = Some(vec![1.0; layer_count]);
        scoring.undesired_direction_trace_cost = Some(vec![1.0; layer_count]);
        for layer in self.layers.as_mut().expect("set above").iter_mut() {
            layer.routable = Some(true);
            layer.preferred_direction_horizontal = None;
            layer.bend_cost = None;
        }
    }


        pub const MIN_BEND_COST: f64 = 0.0;

        pub const MAX_BEND_COST: f64 = 9.9;

        pub const ALGORITHM_CURRENT: &'static str = "freerouting-router";

        pub const ALGORITHM_V19: &'static str = "freerouting-router-v19";

            pub fn get_neck_width_um(&self) -> f64 {
        self.neck_width_um.filter(|v| *v > 0.0).unwrap_or(0.0)
    }

        pub fn is_strict_drc(&self) -> bool {
        self.strict_drc.unwrap_or(false)
    }

        pub fn get_automatic_neckdown(&self) -> bool {
        self.automatic_neckdown.unwrap_or(false)
    }

        pub fn set_automatic_neckdown(&mut self, value: bool) {
        self.automatic_neckdown = Some(value);
    }

            pub fn get_start_ripup_costs(&self) -> i32 {
        self.scoring
            .as_ref()
            .and_then(|s| s.start_ripup_costs)
            .unwrap_or(1)
    }

            pub fn set_start_ripup_costs(&mut self, value: i32) {
        self.scoring
            .get_or_insert_with(ScoringSettings::default)
            .start_ripup_costs = Some(value.max(1));
    }

        pub fn get_run_router(&self) -> bool {
        self.enabled.unwrap_or(true)
    }

        pub fn set_run_router(&mut self, value: bool) {
        self.enabled = Some(value);
    }

                                                                            pub fn is_fanout_enabled(&self) -> bool {
        self.fanout
            .as_ref()
            .and_then(|fanout| fanout.enabled)
            .unwrap_or(false)
    }

                pub fn get_run_optimizer(&self) -> bool {
        self.optimizer
            .as_ref()
            .and_then(|o| o.enabled)
            .unwrap_or(false)
    }

        pub fn set_run_optimizer(&mut self, value: bool) {
        self.optimizer
            .get_or_insert_with(OptimizerSettings::default)
            .enabled = Some(value);
    }

        pub fn get_vias_allowed(&self) -> bool {
        self.vias_allowed.unwrap_or(true)
    }

                    pub fn set_vias_allowed(&mut self, value: Option<bool>) {
        self.vias_allowed = value;
    }

        pub fn get_via_costs(&self) -> i32 {
        self.scoring.as_ref().and_then(|s| s.via_costs).unwrap_or(1)
    }

        pub fn set_via_costs(&mut self, value: i32) {
        self.scoring
            .get_or_insert_with(ScoringSettings::default)
            .via_costs = Some(value.max(1));
    }

        pub fn get_plane_via_costs(&self) -> i32 {
        self.scoring
            .as_ref()
            .and_then(|s| s.plane_via_costs)
            .unwrap_or(1)
    }

        pub fn set_plane_via_costs(&mut self, value: i32) {
        self.scoring
            .get_or_insert_with(ScoringSettings::default)
            .plane_via_costs = Some(value.max(1));
    }

        pub fn set_max_passes(&mut self, value: Option<i32>) {
        self.max_passes = value;
    }

        pub fn set_job_timeout_string(&mut self, value: Option<String>) {
        self.job_timeout_string = value;
    }

        pub fn set_enabled(&mut self, value: Option<bool>) {
        self.enabled = value;
    }


        pub fn set_layer_active(&mut self, layer: usize, value: bool) {
        let Some(entry) = self.layers.as_mut().and_then(|l| l.get_mut(layer)) else {
            return;
        };
        entry.routable = Some(value);
    }

            pub fn get_layer_active(&self, layer: usize) -> bool {
        let Some(entry) = self.layers.as_ref().and_then(|l| l.get(layer)) else {
            return false;
        };
        entry.routable.unwrap_or(true)
    }

            pub fn set_bend_cost(&mut self, layer: usize, value: f64) {
        let Some(entry) = self.layers.as_mut().and_then(|l| l.get_mut(layer)) else {
            return;
        };
        entry.bend_cost = Some(clamp_bend_cost(value));
    }

                                        pub fn get_bend_cost(&self, layer: usize) -> f64 {
        let Some(entry) = self.layers.as_ref().and_then(|l| l.get(layer)) else {
            return 0.0;
        };
        match entry.bend_cost {
            Some(value) => value,
            None => self
                .scoring
                .as_ref()
                .and_then(|s| s.default_bend_cost)
                .map_or(0.0, clamp_bend_cost),
        }
    }

        pub fn set_preferred_direction_is_horizontal(&mut self, layer: usize, value: bool) {
        let Some(entry) = self.layers.as_mut().and_then(|l| l.get_mut(layer)) else {
            return;
        };
        entry.preferred_direction_horizontal = Some(value);
    }

            pub fn get_preferred_direction_is_horizontal(&self, layer: usize) -> bool {
        let Some(entry) = self.layers.as_ref().and_then(|l| l.get(layer)) else {
            return false;
        };
        entry
            .preferred_direction_horizontal
            .unwrap_or(layer % 2 == 1)
    }

                            pub fn set_preferred_direction_trace_costs(&mut self, layer: usize, value: f64) {
        let layer_count = self.get_layer_count();
        if layer >= layer_count {
            return;
        }
        let scoring = self.scoring.get_or_insert_with(ScoringSettings::default);
        if !matches!(&scoring.preferred_direction_trace_cost, Some(a) if a.len() == layer_count) {
            scoring.preferred_direction_trace_cost = Some(vec![0.0; layer_count]);
        }
        scoring
            .preferred_direction_trace_cost
            .as_mut()
            .expect("set above")[layer] = java_math_max(value, 0.1);
        self.board_specific_trace_costs_applied = Some(true);
    }

            pub fn get_preferred_direction_trace_costs(&self, layer: usize) -> f64 {
        if layer >= self.get_layer_count() {
            return 0.0;
        }
        self.scoring
            .as_ref()
            .and_then(|s| s.preferred_direction_trace_cost.as_ref())
            .and_then(|a| a.get(layer).copied())
            .unwrap_or(1.0)
    }

                pub fn set_against_preferred_direction_trace_costs(&mut self, layer: usize, value: f64) {
        let layer_count = self.get_layer_count();
        if layer >= layer_count {
            return;
        }
        let scoring = self.scoring.get_or_insert_with(ScoringSettings::default);
        if !matches!(&scoring.undesired_direction_trace_cost, Some(a) if a.len() == layer_count) {
            scoring.undesired_direction_trace_cost = Some(vec![0.0; layer_count]);
        }
        scoring
            .undesired_direction_trace_cost
            .as_mut()
            .expect("set above")[layer] = java_math_max(value, 0.1);
        self.board_specific_trace_costs_applied = Some(true);
    }

        pub fn get_against_preferred_direction_trace_costs(&self, layer: usize) -> f64 {
        if layer >= self.get_layer_count() {
            return 0.0;
        }
        self.scoring
            .as_ref()
            .and_then(|s| s.undesired_direction_trace_cost.as_ref())
            .and_then(|a| a.get(layer).copied())
            .unwrap_or(1.0)
    }

                                                            pub fn get_horizontal_trace_costs(&self, layer: usize) -> f64 {
        if layer >= self.get_layer_count() {
            return 0.0;
        }
        let array = self.scoring.as_ref().and_then(|scoring| {
            if self.get_preferred_direction_is_horizontal(layer) {
                scoring.preferred_direction_trace_cost.as_ref()
            } else {
                scoring.undesired_direction_trace_cost.as_ref()
            }
        });
        array.and_then(|a| a.get(layer).copied()).unwrap_or(1.0)
    }

                                    pub fn get_vertical_trace_costs(&self, layer: usize) -> f64 {
        if layer >= self.get_layer_count() {
            return 0.0;
        }
        let array = self.scoring.as_ref().and_then(|scoring| {
            if self.get_preferred_direction_is_horizontal(layer) {
                scoring.undesired_direction_trace_cost.as_ref()
            } else {
                scoring.preferred_direction_trace_cost.as_ref()
            }
        });
        array.and_then(|a| a.get(layer).copied()).unwrap_or(1.0)
    }

                                        pub fn get_trace_costs(&self) -> Vec<ExpansionCostFactor> {
        let Some(length) = self
            .scoring
            .as_ref()
            .and_then(|s| s.preferred_direction_trace_cost.as_ref())
            .map(Vec::len)
        else {
            return Vec::new();
        };
        (0..length)
            .map(|i| ExpansionCostFactor {
                horizontal: self.get_horizontal_trace_costs(i),
                vertical: self.get_vertical_trace_costs(i),
            })
            .collect()
    }


                                                                                                    pub fn java_clone(&self) -> Self {
        let mut result = Self::new();
        let layer_count = self.get_layer_count();
        if layer_count > 0 {
            result.set_layer_count(layer_count);
        }
        result.algorithm = self.algorithm.clone();
        result.job_timeout_string = self.job_timeout_string.clone();
        if self.layers.is_some() {
            result.layers = self.layers.clone();
        }
        result.max_passes = self.max_passes;
        result.max_items = self.max_items;
        result.save_intermediate_stages = self.save_intermediate_stages;
        result.copper_to_edge_clearance_um = self.copper_to_edge_clearance_um;
        result.hole_clearance_um = self.hole_clearance_um;
        result.neck_width_um = self.neck_width_um;
        result.strict_drc = self.strict_drc;
        result.ignore_net_classes = self.ignore_net_classes.clone();
        result.trace_pull_tight_accuracy = self.trace_pull_tight_accuracy;
        result.enabled = self.enabled;
        result.vias_allowed = self.vias_allowed;
        result.automatic_neckdown = self.automatic_neckdown;
        result.max_threads = self.max_threads;
        result.optimizer = Some(self.optimizer.clone().unwrap_or_default());
        result.scoring = Some(self.scoring.clone().unwrap_or_default());
        result.fanout = Some(self.fanout.clone().unwrap_or_default());
        result.board_specific_trace_costs_applied = self.board_specific_trace_costs_applied;
        result
    }

                            pub fn set_max_threads(&mut self, value: Option<i32>, host: &HostEnvironment) {
        let normalized = normalize_max_threads(value, host);
        self.max_threads = Some(normalized);
        if let Some(optimizer) = self.optimizer.as_mut() {
            optimizer.max_threads = Some(normalized);
        }
    }

                                                                                                                            pub fn validate(&mut self, host: &HostEnvironment) {
        let max_passes = self
            .max_passes
            .expect("RouterSettings.java:934: maxPasses is dereferenced unboxed");
        if !(0..=9999).contains(&max_passes) {
            self.max_passes = Some(9999);
        } else if max_passes == 0 {
            self.max_passes = Some(i32::MAX);
        }

        let available_processors = host.available_processors() as i32;
        match self.max_threads {
            None => self.max_threads = Some(host.default_max_threads()),
            Some(value) if value < 0 => self.max_threads = Some(host.default_max_threads()),
            Some(value) if value > available_processors => {
                self.max_threads = Some(available_processors);
            }
            Some(_) => {}
        }

        let accuracy = self
            .trace_pull_tight_accuracy
            .expect("RouterSettings.java:958: tracePullTightAccuracy is dereferenced unboxed");
        if accuracy < 1 {
            self.trace_pull_tight_accuracy = Some(500);
        }
    }
}

fn normalize_max_threads(value: Option<i32>, host: &HostEnvironment) -> i32 {
    match value {
        None => host.default_max_threads(),
        Some(value) if value < 0 => host.default_max_threads(),
        Some(0) => host.available_processors() as i32,
        Some(value) => value.min(host.available_processors() as i32),
    }
}

fn clamp_bend_cost(value: f64) -> f64 {
    value.clamp(RouterSettings::MIN_BEND_COST, RouterSettings::MAX_BEND_COST)
}

fn java_math_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.max(b)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct ExpansionCostFactor {
        pub horizontal: f64,
        pub vertical: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

                                                                    #[test]
    fn set_layer_count_rewipes_costs_but_keeps_the_applied_flag() {
        let mut settings = RouterSettings::new();
        settings.set_layer_count(2);
        settings.set_preferred_direction_trace_costs(0, 2.5);
        assert_eq!(settings.board_specific_trace_costs_applied, Some(true));

        settings.set_layer_count(2);

        assert_eq!(settings.get_preferred_direction_trace_costs(0), 1.0);
        assert_eq!(
            settings.board_specific_trace_costs_applied,
            Some(true),
            "the :456 reallocation branch did not fire, so :457 did not clear the flag"
        );
    }

                #[test]
    fn set_layer_count_resets_applied_flag() {
        let mut settings = RouterSettings::new();
        settings.set_layer_count(2);
        settings.set_preferred_direction_trace_costs(0, 2.5);
        assert_eq!(settings.board_specific_trace_costs_applied, Some(true));

        settings.set_layer_count(4);

        assert_eq!(settings.board_specific_trace_costs_applied, Some(false));
        assert_eq!(settings.get_layer_count(), 4);
    }

            #[test]
    fn out_of_range_trace_cost_setter_does_not_set_the_applied_flag() {
        let mut settings = RouterSettings::new();
        settings.set_preferred_direction_trace_costs(0, 9.0);
        assert_eq!(settings.board_specific_trace_costs_applied, None);
        assert_eq!(
            settings
                .scoring
                .as_ref()
                .unwrap()
                .preferred_direction_trace_cost,
            None
        );
    }

            #[test]
    fn java_clone_carries_the_applied_flag() {
        let mut settings = RouterSettings::new();
        settings.set_layer_count(2);
        settings.set_preferred_direction_trace_costs(0, 2.5);
        assert_eq!(
            settings.java_clone().board_specific_trace_costs_applied,
            Some(true)
        );

        let mut untouched = RouterSettings::new();
        untouched.set_layer_count(2);
        assert_eq!(
            untouched.java_clone().board_specific_trace_costs_applied,
            Some(false)
        );
    }

        #[test]
    fn java_math_max_propagates_nan() {
        assert!(java_math_max(f64::NAN, 0.1).is_nan());
        assert!(java_math_max(0.1, f64::NAN).is_nan());
        assert_eq!(java_math_max(0.05, 0.1), 0.1);
        assert_eq!(java_math_max(5.0, 0.1), 5.0);
    }
}
