use crate::cancel::{CancelToken, JobStopReason};
use crate::progress::SyncProgressSink;

#[derive(Debug)]
pub struct Ctx<'a> {
    pub settings: &'a fr_settings::RouterSettings,
    pub cancel: CancelToken,
    pub progress: &'a SyncProgressSink,
    pub budget: fr_router::pipeline::RouterBudget,
}

impl<'a> Ctx<'a> {
    pub fn new(
        settings: &'a fr_settings::RouterSettings,
        progress: &'a SyncProgressSink,
    ) -> Ctx<'a> {
        Ctx {
            settings,
            cancel: CancelToken::new(),
            progress,
            budget: fr_router::pipeline::RouterBudget::default(),
        }
    }

    pub fn with_disabled_budget(
        settings: &'a fr_settings::RouterSettings,
        progress: &'a SyncProgressSink,
    ) -> Ctx<'a> {
        Ctx {
            budget: fr_router::pipeline::RouterBudget::disabled(),
            ..Ctx::new(settings, progress)
        }
    }
}

#[derive(Debug, Clone)]
pub struct RoutingResult {
    pub stats: fr_router::score::BoardStatistics,
    pub unrouted_report: String,
    pub drc_violations: Vec<fr_drc::DrcViolation>,
    pub timed_out: bool,
    pub stop_reason: Option<JobStopReason>,
    pub pipeline: fr_router::pipeline::PipelineResult,
}

impl RoutingResult {
    pub fn incomplete_count(&self) -> Option<i32> {
        self.stats.connections.incomplete_count
    }

    pub fn violation_count(&self) -> usize {
        self.drc_violations.len()
    }
}
