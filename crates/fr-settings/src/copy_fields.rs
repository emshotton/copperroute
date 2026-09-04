use crate::{
    BoardUpdateStrategy, DebugSettings, DesignRulesCheckerSettings, FanoutSettings,
    ItemSelectionStrategy, LayerSettings, MergeError, MergeReport, OptimizerSettings,
    RouterSettings, ScoringSettings,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeMode {
        Overwrite,
        FillAbsent,
}

pub trait CopyFields {
        fn merge_fields_into(&self, target: &mut Self, mode: MergeMode, report: &mut MergeReport);

            fn copy_fields_into(&self, target: &mut Self, report: &mut MergeReport) {
        self.merge_fields_into(target, MergeMode::Overwrite, report);
    }
}

pub trait JavaEnum: Copy + Sized {
        fn java_name(self) -> &'static str;

            fn from_java_name(name: &str) -> Option<Self>;
}

impl JavaEnum for BoardUpdateStrategy {
    fn java_name(self) -> &'static str {
        BoardUpdateStrategy::java_name(self)
    }

    fn from_java_name(name: &str) -> Option<Self> {
        match name {
            "GREEDY" => Some(Self::Greedy),
            "GLOBAL_OPTIMAL" => Some(Self::GlobalOptimal),
            "HYBRID" => Some(Self::Hybrid),
            _ => None,
        }
    }
}

impl JavaEnum for ItemSelectionStrategy {
    fn java_name(self) -> &'static str {
        ItemSelectionStrategy::java_name(self)
    }

    fn from_java_name(name: &str) -> Option<Self> {
        match name {
            "SEQUENTIAL" => Some(Self::Sequential),
            "RANDOM" => Some(Self::Random),
            "PRIORITIZED" => Some(Self::Prioritized),
            _ => None,
        }
    }
}


pub fn scalar_copy<T: PartialEq + Clone>(
    src: &Option<T>,
    dst: &mut Option<T>,
    mode: MergeMode,
    report: &mut MergeReport,
) {
    let Some(value) = src else {
        return; 
    };
    if mode == MergeMode::FillAbsent && dst.is_some() {
        return;
    }
    *dst = Some(value.clone());
    report.fields_changed += 1;
}

pub fn enum_copy<T: JavaEnum>(
    path: &str,
    src: &Option<T>,
    dst: &mut Option<T>,
    mode: MergeMode,
    report: &mut MergeReport,
) {
    let Some(value) = src else {
        return;
    };
    if mode == MergeMode::FillAbsent && dst.is_some() {
        return;
    }
    enum_copy_by_name(path, value.java_name(), dst, report);
}

pub fn enum_copy_by_name<T: JavaEnum>(
    path: &str,
    name: &str,
    dst: &mut Option<T>,
    report: &mut MergeReport,
) {
    match T::from_java_name(name) {
        Some(parsed) => {
            *dst = Some(parsed);
            report.fields_changed += 1;
        }
        None => report.errors.push(MergeError::EnumName {
            path: path.to_string(),
            value: name.to_string(),
        }),
    }
}

pub fn primitive_array_copy<T: Clone>(
    src: &Option<Vec<T>>,
    dst: &mut Option<Vec<T>>,
    report: &mut MergeReport,
) {
    let Some(source) = src else {
        return; 
    };
    let should_copy = match dst {
        None => true,                                            
        Some(target) => target.is_empty() && !source.is_empty(), 
    };
    if should_copy {
        *dst = Some(source.clone());
        report.fields_changed += 1;
    }
}

pub fn primitive_array_copy_plain<T: Clone>(src: &[T], dst: &mut Vec<T>, report: &mut MergeReport) {
    if dst.is_empty() && !src.is_empty() {
        *dst = src.to_vec();
        report.fields_changed += 1;
    }
}

pub fn object_array_merge<T: CopyFields + Default + Clone>(
    src: &Option<Vec<T>>,
    dst: &mut Option<Vec<T>>,
    mode: MergeMode,
    report: &mut MergeReport,
) {
    let Some(source) = src else {
        return; 
    };
    let mut inner = MergeReport::default();
    let target_len = dst.as_ref().map_or(0, Vec::len);
    if target_len >= source.len() {
        if let Some(target) = dst.as_mut() {
            for (element, slot) in source.iter().zip(target.iter_mut()) {
                element.merge_fields_into(slot, mode, &mut inner);
            }
        }
    } else if mode == MergeMode::FillAbsent && dst.is_some() {
        let target = dst.as_mut().expect("is_some checked above");
        target.resize_with(source.len(), T::default);
        for (element, slot) in source.iter().zip(target.iter_mut()) {
            element.merge_fields_into(slot, mode, &mut inner);
        }
    } else {
        let mut fresh = Vec::with_capacity(source.len());
        for element in source {
            let mut slot = T::default();
            element.merge_fields_into(&mut slot, mode, &mut inner);
            fresh.push(slot);
        }
        *dst = Some(fresh);
    }
    report.errors.append(&mut inner.errors);
    report.fields_changed += source.len();
}

pub fn nested_copy<T: CopyFields + Default>(
    src: &Option<T>,
    dst: &mut Option<T>,
    mode: MergeMode,
    report: &mut MergeReport,
) {
    let Some(source) = src else {
        return; 
    };
    let target = dst.get_or_insert_with(T::default);
    source.merge_fields_into(target, mode, report);
}

pub fn primitive_bool_copy(src: bool, dst: &mut bool, report: &mut MergeReport) {
    if !src {
        return;
    }
    *dst = true;
    report.fields_changed += 1;
}

pub fn primitive_i32_copy(src: i32, dst: &mut i32, report: &mut MergeReport) {
    if src == 0 {
        return;
    }
    *dst = src;
    report.fields_changed += 1;
}


impl CopyFields for LayerSettings {
        fn merge_fields_into(&self, target: &mut Self, mode: MergeMode, report: &mut MergeReport) {
        scalar_copy(&self.routable, &mut target.routable, mode, report);
        scalar_copy(
            &self.preferred_direction_horizontal,
            &mut target.preferred_direction_horizontal,
            mode,
            report,
        );
        scalar_copy(&self.bend_cost, &mut target.bend_cost, mode, report);
    }
}

impl CopyFields for ScoringSettings {
        fn merge_fields_into(&self, target: &mut Self, mode: MergeMode, report: &mut MergeReport) {
        primitive_array_copy(
            &self.preferred_direction_trace_cost,
            &mut target.preferred_direction_trace_cost,
            report,
        );
        primitive_array_copy(
            &self.undesired_direction_trace_cost,
            &mut target.undesired_direction_trace_cost,
            report,
        );
        scalar_copy(
            &self.default_preferred_direction_trace_cost,
            &mut target.default_preferred_direction_trace_cost,
            mode,
            report,
        );
        scalar_copy(
            &self.default_undesired_direction_trace_cost,
            &mut target.default_undesired_direction_trace_cost,
            mode,
            report,
        );
        scalar_copy(&self.via_costs, &mut target.via_costs, mode, report);
        scalar_copy(
            &self.plane_via_costs,
            &mut target.plane_via_costs,
            mode,
            report,
        );
        scalar_copy(
            &self.start_ripup_costs,
            &mut target.start_ripup_costs,
            mode,
            report,
        );
        scalar_copy(
            &self.unrouted_net_penalty,
            &mut target.unrouted_net_penalty,
            mode,
            report,
        );
        scalar_copy(
            &self.clearance_violation_penalty,
            &mut target.clearance_violation_penalty,
            mode,
            report,
        );
        scalar_copy(&self.bend_penalty, &mut target.bend_penalty, mode, report);
        scalar_copy(
            &self.default_bend_cost,
            &mut target.default_bend_cost,
            mode,
            report,
        );
    }
}

impl CopyFields for OptimizerSettings {
            fn merge_fields_into(&self, target: &mut Self, mode: MergeMode, report: &mut MergeReport) {
        scalar_copy(&self.enabled, &mut target.enabled, mode, report);
        scalar_copy(&self.algorithm, &mut target.algorithm, mode, report);
        scalar_copy(&self.max_passes, &mut target.max_passes, mode, report);
        scalar_copy(&self.max_items, &mut target.max_items, mode, report);
        scalar_copy(&self.max_threads, &mut target.max_threads, mode, report);
        scalar_copy(
            &self.optimization_improvement_threshold,
            &mut target.optimization_improvement_threshold,
            mode,
            report,
        );
        scalar_copy(
            &self.max_consecutive_failures,
            &mut target.max_consecutive_failures,
            mode,
            report,
        );
        scalar_copy(
            &self.additional_ripup_cost_factor_at_start,
            &mut target.additional_ripup_cost_factor_at_start,
            mode,
            report,
        );
        scalar_copy(
            &self.trace_ripup_cost_factor,
            &mut target.trace_ripup_cost_factor,
            mode,
            report,
        );
        scalar_copy(
            &self.max_autoroute_passes,
            &mut target.max_autoroute_passes,
            mode,
            report,
        );
        enum_copy(
            "board_update_strategy",
            &self.board_update_strategy,
            &mut target.board_update_strategy,
            mode,
            report,
        );
        scalar_copy(&self.hybrid_ratio, &mut target.hybrid_ratio, mode, report);
        enum_copy(
            "item_selection_strategy",
            &self.item_selection_strategy,
            &mut target.item_selection_strategy,
            mode,
            report,
        );
        scalar_copy(
            &self.timeout_string,
            &mut target.timeout_string,
            mode,
            report,
        );
    }
}

impl CopyFields for FanoutSettings {
        fn merge_fields_into(&self, target: &mut Self, mode: MergeMode, report: &mut MergeReport) {
        scalar_copy(&self.enabled, &mut target.enabled, mode, report);
        scalar_copy(&self.max_passes, &mut target.max_passes, mode, report);
        scalar_copy(&self.max_items, &mut target.max_items, mode, report);
        scalar_copy(
            &self.max_milliseconds_per_pin,
            &mut target.max_milliseconds_per_pin,
            mode,
            report,
        );
        scalar_copy(&self.ripup_allowed, &mut target.ripup_allowed, mode, report);
        scalar_copy(
            &self.min_escape_length_mm,
            &mut target.min_escape_length_mm,
            mode,
            report,
        );
        scalar_copy(
            &self.max_escape_length_mm,
            &mut target.max_escape_length_mm,
            mode,
            report,
        );
        scalar_copy(
            &self.start_via_diameter_mm,
            &mut target.start_via_diameter_mm,
            mode,
            report,
        );
        scalar_copy(
            &self.end_via_diameter_mm,
            &mut target.end_via_diameter_mm,
            mode,
            report,
        );
        scalar_copy(
            &self.pin_sorting_order,
            &mut target.pin_sorting_order,
            mode,
            report,
        );
        scalar_copy(
            &self.fallback_to_board_vias,
            &mut target.fallback_to_board_vias,
            mode,
            report,
        );
        scalar_copy(
            &self.timeout_string,
            &mut target.timeout_string,
            mode,
            report,
        );
    }
}

impl CopyFields for DesignRulesCheckerSettings {
            fn merge_fields_into(&self, target: &mut Self, _mode: MergeMode, report: &mut MergeReport) {
        primitive_bool_copy(self.enabled, &mut target.enabled, report);
        primitive_bool_copy(self.include_warnings, &mut target.include_warnings, report);
        primitive_bool_copy(self.include_errors, &mut target.include_errors, report);
    }
}

impl CopyFields for DebugSettings {
            fn merge_fields_into(&self, target: &mut Self, _mode: MergeMode, report: &mut MergeReport) {
        primitive_bool_copy(
            self.enable_detailed_logging,
            &mut target.enable_detailed_logging,
            report,
        );
        primitive_bool_copy(
            self.single_step_execution,
            &mut target.single_step_execution,
            report,
        );
        primitive_i32_copy(
            self.trace_insertion_delay,
            &mut target.trace_insertion_delay,
            report,
        );
        let _ = &self.filter_by_net;
        primitive_array_copy_plain(
            &self.operation_filters,
            &mut target.operation_filters,
            report,
        );
    }
}

impl CopyFields for RouterSettings {
                fn merge_fields_into(&self, target: &mut Self, mode: MergeMode, report: &mut MergeReport) {
        scalar_copy(&self.enabled, &mut target.enabled, mode, report);
        scalar_copy(&self.algorithm, &mut target.algorithm, mode, report);
        nested_copy(&self.fanout, &mut target.fanout, mode, report);
        scalar_copy(
            &self.copper_to_edge_clearance_um,
            &mut target.copper_to_edge_clearance_um,
            mode,
            report,
        );
        scalar_copy(
            &self.hole_clearance_um,
            &mut target.hole_clearance_um,
            mode,
            report,
        );
        scalar_copy(&self.neck_width_um, &mut target.neck_width_um, mode, report);
        scalar_copy(&self.strict_drc, &mut target.strict_drc, mode, report);
        scalar_copy(
            &self.job_timeout_string,
            &mut target.job_timeout_string,
            mode,
            report,
        );
        scalar_copy(&self.max_passes, &mut target.max_passes, mode, report);
        scalar_copy(&self.max_items, &mut target.max_items, mode, report);
        object_array_merge(&self.layers, &mut target.layers, mode, report);
        scalar_copy(
            &self.save_intermediate_stages,
            &mut target.save_intermediate_stages,
            mode,
            report,
        );
        primitive_array_copy(
            &self.ignore_net_classes,
            &mut target.ignore_net_classes,
            report,
        );
        scalar_copy(
            &self.trace_pull_tight_accuracy,
            &mut target.trace_pull_tight_accuracy,
            mode,
            report,
        );
        scalar_copy(&self.vias_allowed, &mut target.vias_allowed, mode, report);
        scalar_copy(
            &self.automatic_neckdown,
            &mut target.automatic_neckdown,
            mode,
            report,
        );
        nested_copy(&self.optimizer, &mut target.optimizer, mode, report);
        nested_copy(&self.scoring, &mut target.scoring, mode, report);
        scalar_copy(&self.max_threads, &mut target.max_threads, mode, report);
        scalar_copy(
            &self.result_json_path,
            &mut target.result_json_path,
            mode,
            report,
        );
        scalar_copy(
            &self.opt_changed_area_ms,
            &mut target.opt_changed_area_ms,
            mode,
            report,
        );
    }
}

impl RouterSettings {
                            pub fn apply_new_values_from(&mut self, source: &RouterSettings) -> MergeReport {
        let mut report = MergeReport::default();
        source.copy_fields_into(self, &mut report);
        report
    }

                                pub fn fill_absent_from(&mut self, source: &RouterSettings) -> MergeReport {
        let mut report = MergeReport::default();
        source.merge_fields_into(self, MergeMode::FillAbsent, &mut report);
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

                                #[test]
    fn board_specific_flag_is_never_copied() {
        let mut source = RouterSettings::new();
        source.board_specific_trace_costs_applied = Some(true);
        let mut target = RouterSettings::new();
        assert_eq!(target.board_specific_trace_costs_applied, None);

        target.apply_new_values_from(&source);

        assert_eq!(target.board_specific_trace_costs_applied, None);

        target.fill_absent_from(&source);
        assert_eq!(target.board_specific_trace_costs_applied, None);
    }
}
