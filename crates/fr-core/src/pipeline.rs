use fr_board::Board;

use crate::Error;
use crate::ctx::{Ctx, RoutingResult};

#[derive(Debug, Clone, Copy, Default)]
pub struct RoutingPipeline;

impl RoutingPipeline {
    pub fn run(board: &mut Board, ctx: &Ctx<'_>) -> Result<RoutingResult, Error> {
        let stop = ctx.cancel.as_router_stop();

        let mut sink = ctx.progress.as_pipeline_sink();
        let pipeline =
            fr_router::pipeline::run_pipeline(board, ctx.settings, &stop, ctx.budget, &mut sink)?;

        let unrouted_report = fr_router::pipeline::build_unrouted_report(board);

        let drc_violations = fr_drc::DesignRulesChecker::new(board).get_all_violations();

        let timed_out = pipeline.timed_out || stop.is_timed_out() || ctx.cancel.is_timed_out();

        Ok(RoutingResult {
            stats: pipeline.final_statistics.clone(),
            unrouted_report,
            drc_violations,
            timed_out,
            pipeline,
        })
    }
}
