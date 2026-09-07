use copper_board::Board;

use crate::Error;
use crate::cancel::JobStopReason;
use crate::ctx::{Ctx, RoutingResult};

#[derive(Debug, Clone, Copy, Default)]
pub struct RoutingPipeline;

impl RoutingPipeline {
    pub fn run(board: &mut Board, ctx: &Ctx<'_>) -> Result<RoutingResult, Error> {
        let stop = ctx.cancel.as_router_stop();

        let mut sink = ctx.progress.as_pipeline_sink();
        let pipeline = copper_router::pipeline::run_pipeline(
            board,
            ctx.settings,
            &stop,
            ctx.budget,
            &mut sink,
        )?;

        let unrouted_report = copper_router::pipeline::build_unrouted_report(board);

        let drc_violations = copper_drc::DesignRulesChecker::new(board).get_all_violations();

        stop.poll_deadline();
        let stop_reason = if stop.is_timed_out() {
            Some(JobStopReason::Deadline)
        } else if ctx.cancel.is_cancelled() {
            Some(JobStopReason::Cancelled)
        } else {
            None
        };
        let timed_out = stop_reason == Some(JobStopReason::Deadline);

        Ok(RoutingResult {
            stats: pipeline.final_statistics.clone(),
            unrouted_report,
            drc_violations,
            timed_out,
            stop_reason,
            pipeline,
        })
    }
}
